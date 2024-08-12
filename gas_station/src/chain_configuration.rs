use std::cmp::Ordering;

use ethers_core::types::U256;
use lib::{foreign_address::ForeignAddress, pyth};
use near_sdk::{json_types::U128, near};

use crate::{
    error::{
        ConfidenceIntervalTooLargeError, ExponentTooLargeError, NegativePriceError,
        NoPaymasterConfigurationForChainError, PaymasterInsufficientFundsError, PriceDataError,
        RequestNonceError,
    },
    valid_transaction_request::ValidTransactionRequest,
    ExpressionOverflowError, NonceOverflowError,
};

#[derive(Clone, Debug, PartialEq, Eq)]
#[near(serializers = [borsh, json])]
pub struct PaymasterConfiguration {
    pub nonce: u32,
    pub token_id: String,
    pub minimum_available_balance: [u64; 4],
}

impl PaymasterConfiguration {
    /// Deducts `amount` from the paymaster's minimum available balance,
    /// returning the new balance.
    ///
    /// # Errors
    ///
    /// - If `amount` is greater than the paymaster's minimum available balance.
    pub fn sub_from_minimum_available_balance(
        &self,
        amount: U256,
    ) -> Result<U256, PaymasterInsufficientFundsError> {
        U256(self.minimum_available_balance)
            .checked_sub(amount)
            .ok_or(PaymasterInsufficientFundsError {
                minimum_available_balance: U256(self.minimum_available_balance),
                amount,
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[near(serializers = [json])]
pub struct ViewPaymasterConfiguration {
    pub nonce: u32,
    pub token_id: String,
    pub foreign_address: ForeignAddress,
    pub minimum_available_balance: U128,
}

#[derive(Debug)]
#[near]
pub struct ForeignChainConfiguration {
    pub chain_id: u64,
    pub paymasters: near_sdk::collections::TreeMap<String, PaymasterConfiguration>,
    pub next_paymaster: String,
    pub transfer_gas: [u64; 4],
    pub fee_rate: (u128, u128),
    pub oracle_asset_id: [u8; 32],
    pub decimals: u8,
}

impl ForeignChainConfiguration {
    pub fn transfer_gas(&self) -> U256 {
        U256(self.transfer_gas)
    }

    fn next_paymaster_key(&self) -> Option<String> {
        self.paymasters
            .ceil_key(&self.next_paymaster)
            .or_else(|| self.paymasters.min())
    }

    fn paymaster_key_after(&self, key: &String) -> Option<String> {
        self.paymasters
            .higher(key)
            .or_else(|| self.paymasters.min())
    }

    /// Facilitates the "purchase" of a transaction nonce from a paymaster,
    /// ensuring sufficient balance on the foreign chain, proper token key
    /// rotation, etc.
    ///
    /// The predicate will not run if any errors are encountered.
    ///
    /// State is only modified after execution of the predicate.
    ///
    /// # Errors
    ///
    /// - If no paymaster configuration exists.
    /// - If the paymaster has insufficient balance.
    pub fn with_request_nonce<R>(
        &mut self,
        deduct_amount: U256,
        f: impl FnOnce(&Self, &PaymasterConfiguration) -> R,
    ) -> Result<R, RequestNonceError> {
        let (mut paymaster_config, paymaster_key, paymaster_key_after) = self
            .next_paymaster()
            .ok_or(NoPaymasterConfigurationForChainError {
                chain_id: self.chain_id,
            })?;

        let new_minimum_balance =
            paymaster_config.sub_from_minimum_available_balance(deduct_amount)?;

        let r = f(self, &paymaster_config);

        paymaster_config.nonce = paymaster_config
            .nonce
            .checked_add(1)
            .ok_or(NonceOverflowError)?;
        paymaster_config.minimum_available_balance = new_minimum_balance.0;
        self.paymasters.insert(&paymaster_key, &paymaster_config);
        self.next_paymaster = paymaster_key_after;

        Ok(r)
    }

    fn next_paymaster(&self) -> Option<(PaymasterConfiguration, String, String)> {
        let paymaster_key = self.next_paymaster_key()?;
        let paymaster_key_after = self.paymaster_key_after(&paymaster_key)?;
        self.paymasters
            .get(&paymaster_key)
            .map(|c| (c, paymaster_key, paymaster_key_after))
    }

    /// Calculate the gas tokens that this chain configuration charges to
    /// sponsor this transaction.
    ///
    /// # Errors
    ///
    /// - If the calculation overflows U256.
    pub fn calculate_gas_tokens_to_sponsor_transaction(
        &self,
        transaction: &ValidTransactionRequest,
    ) -> Result<U256, ExpressionOverflowError> {
        transaction
            .gas()
            .checked_add(U256(self.transfer_gas))
            .and_then(|x| x.checked_mul(transaction.max_fee_per_gas()))
            .ok_or(ExpressionOverflowError)
    }

    /// Calculate the price that this chain configuration charges to convert
    /// assets. Applies a fee on top of the provided market rates.
    ///
    /// # Errors
    ///
    /// - If the price data is invalid (negative, confidence interval too large).
    pub fn price_for_gas_tokens(
        &self,
        quantity_to_convert: U256,
        this_asset_price_in_usd: &pyth::Price,
        into_asset_price_in_usd: &pyth::Price,
        into_asset_decimals: u8,
    ) -> Result<u128, PriceDataError> {
        // Construct conversion rate
        let mut conversion_rate = (
            u128::try_from(this_asset_price_in_usd.price.0)
                .map_err(|_| NegativePriceError)?
                .checked_sub(
                    // Pessimistic pricing for the asset we're converting from. (Assume it is less valuable.)
                    u128::from(this_asset_price_in_usd.conf.0),
                )
                .ok_or(ConfidenceIntervalTooLargeError)?,
            u128::try_from(into_asset_price_in_usd.price.0)
                .map_err(|_| NegativePriceError)?
                .checked_add(
                    // Pessimistic pricing for the asset we're converting into. (Assume it is more valuable.)
                    u128::from(into_asset_price_in_usd.conf.0),
                )
                .ok_or(ConfidenceIntervalTooLargeError)?,
        );

        let exp = this_asset_price_in_usd
            .expo
            .checked_sub(into_asset_price_in_usd.expo)
            .and_then(|x| x.checked_add(i32::from(into_asset_decimals)))
            .and_then(|x| x.checked_sub(i32::from(self.decimals)))
            .ok_or(ExponentTooLargeError)?;

        // Apply exponent
        match exp.cmp(&0) {
            Ordering::Less => {
                let factor = 10u128
                    .checked_pow(exp.unsigned_abs())
                    .ok_or(ExponentTooLargeError)?;
                conversion_rate.1 = conversion_rate
                    .1
                    .checked_mul(factor)
                    .ok_or(ExponentTooLargeError)?;
            }
            #[allow(clippy::cast_sign_loss)]
            Ordering::Greater => {
                let factor = 10u128
                    .checked_pow(exp as u32)
                    .ok_or(ExponentTooLargeError)?;
                conversion_rate.0 = conversion_rate
                    .0
                    .checked_mul(factor)
                    .ok_or(ExponentTooLargeError)?;
            }
            Ordering::Equal => {}
        }

        // Apply conversion rate to quantity in two steps: multiply, then divide.
        let numerator = quantity_to_convert
            .checked_mul(U256::from(conversion_rate.0))
            .and_then(|x| x.checked_mul(U256::from(self.fee_rate.0)))
            .ok_or(ExpressionOverflowError)?;
        let denominator = U256::from(conversion_rate.1)
            .checked_mul(U256::from(self.fee_rate.1))
            .ok_or(ExpressionOverflowError)?;
        let (b, rem) = numerator.div_mod(denominator);

        // Round up. Again, pessimistic pricing.
        Ok(if rem.is_zero() {
            b
        } else {
            // It should be impossible for this to overflow, given the above calculations, but better safe than sorry.
            b.checked_add(U256::one()).ok_or(ExpressionOverflowError)?
        }
        .as_u128())
    }
}

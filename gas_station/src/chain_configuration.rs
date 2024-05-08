use std::cmp::Ordering;

use ethers_core::types::U256;
use lib::{foreign_address::ForeignAddress, pyth};
use near_sdk::{json_types::U128, near};

use crate::{
    error::{
        ConfidenceIntervalTooLargeError, ExponentTooLargeError, NegativePriceError,
        PaymasterInsufficientFundsError, PriceDataError,
    },
    valid_transaction_request::ValidTransactionRequest,
};

#[derive(Clone, Debug, PartialEq, Eq)]
#[near(serializers = [borsh, json])]
pub struct PaymasterConfiguration {
    pub nonce: u32,
    pub token_id: String,
    pub minimum_available_balance: [u64; 4],
}

impl PaymasterConfiguration {
    pub fn next_nonce(&mut self) -> u32 {
        let nonce = self.nonce;
        self.nonce += 1;
        nonce
    }

    /// Deducts `amount` from the paymaster's minimum available balance.
    ///
    /// # Errors
    ///
    /// - If `amount` is greater than the paymaster's minimum available balance.
    pub fn deduct(&mut self, amount: U256) -> Result<(), PaymasterInsufficientFundsError> {
        self.minimum_available_balance = U256(self.minimum_available_balance)
            .checked_sub(amount)
            .ok_or(PaymasterInsufficientFundsError {
                minimum_available_balance: U256(self.minimum_available_balance),
                amount,
            })?
            .0;
        Ok(())
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

    pub fn next_paymaster(&mut self) -> Option<PaymasterConfiguration> {
        let paymaster_key = self
            .paymasters
            .ceil_key(&self.next_paymaster)
            .or_else(|| self.paymasters.min())?;
        let next_paymaster_key = self
            .paymasters
            .higher(&paymaster_key)
            .or_else(|| self.paymasters.min())?;
        self.next_paymaster = next_paymaster_key;
        self.paymasters.get(&paymaster_key)
    }

    pub fn calculate_gas_tokens_to_sponsor_transaction(
        &self,
        transaction: &ValidTransactionRequest,
    ) -> U256 {
        (transaction.gas() + U256(self.transfer_gas)) * transaction.max_fee_per_gas()
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
        let a = quantity_to_convert * U256::from(conversion_rate.0) * U256::from(self.fee_rate.0);
        let (b, rem) = a.div_mod(U256::from(conversion_rate.1) * U256::from(self.fee_rate.1));

        // Round up. Again, pessimistic pricing.
        Ok(if rem.is_zero() { b } else { b + 1 }.as_u128())
    }
}

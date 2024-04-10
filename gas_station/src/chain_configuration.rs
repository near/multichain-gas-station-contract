use ethers_core::types::U256;
use lib::{
    foreign_address::ForeignAddress,
    oracle::{process_oracle_result, PriceData},
    Rejectable,
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    json_types::U128,
    serde::{Deserialize, Serialize},
};
use schemars::JsonSchema;
use thiserror::Error;

#[derive(
    Serialize,
    Deserialize,
    BorshSerialize,
    BorshDeserialize,
    JsonSchema,
    Clone,
    Debug,
    PartialEq,
    Eq,
)]
#[serde(crate = "near_sdk::serde")]
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

    pub fn deduct(&mut self, request_tokens_for_gas: U256) {
        self.minimum_available_balance = U256(self.minimum_available_balance)
            .checked_sub(request_tokens_for_gas)
            .expect_or_reject("Paymaster does not have enough funds")
            .0;
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
#[serde(crate = "near_sdk::serde")]
pub struct ViewPaymasterConfiguration {
    pub nonce: u32,
    pub token_id: String,
    pub foreign_address: ForeignAddress,
    pub minimum_available_balance: U128,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct ChainConfiguration {
    pub paymasters: near_sdk::collections::TreeMap<String, PaymasterConfiguration>,
    pub next_paymaster: String,
    pub transfer_gas: [u64; 4],
    pub fee_rate: (u128, u128),
    pub oracle_asset_id: String,
}

#[derive(Debug, Error)]
#[error("Paymaster with index {0} does not exist")]
pub struct PaymasterDoesNotExistError(u32);

impl ChainConfiguration {
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

    pub fn foreign_token_price(
        &self,
        oracle_local_asset_id: &str,
        price_data: &PriceData,
        foreign_tokens: U256,
    ) -> u128 {
        let foreign_token_price =
            process_oracle_result(oracle_local_asset_id, &self.oracle_asset_id, price_data);

        // calculate fee based on currently known price, and include fee rate
        let a = foreign_tokens * U256::from(foreign_token_price.0) * U256::from(self.fee_rate.0);
        let (b, rem) = a.div_mod(U256::from(foreign_token_price.1) * U256::from(self.fee_rate.1));
        // round up
        if rem.is_zero() { b } else { b + 1 }.as_u128()
    }
}

use ethers_core::types::U256;
use lib::{
    foreign_address::ForeignAddress,
    oracle::{process_oracle_result, PriceData},
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    json_types::U128,
    serde::{Deserialize, Serialize},
    store::Vector,
};
use schemars::JsonSchema;

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
    pub key_path: String,
    pub minimum_available_balance: [u64; 4],
}

impl PaymasterConfiguration {
    pub fn next_nonce(&mut self) -> u32 {
        let nonce = self.nonce;
        self.nonce += 1;
        nonce
    }
}

#[derive(Serialize, JsonSchema, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct ViewPaymasterConfiguration {
    pub nonce: u32,
    pub key_path: String,
    pub foreign_address: ForeignAddress,
    pub minimum_available_balance: U128,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct ChainConfiguration {
    pub paymasters: Vector<PaymasterConfiguration>,
    pub next_paymaster: u32,
    pub transfer_gas: [u64; 4],
    pub fee_rate: (u128, u128),
    pub oracle_asset_id: String,
}

impl ChainConfiguration {
    pub fn transfer_gas(&self) -> U256 {
        U256(self.transfer_gas)
    }

    pub fn next_paymaster(&mut self) -> Option<&mut PaymasterConfiguration> {
        let next_paymaster = self.next_paymaster % self.paymasters.len(); // so that each individual paymaster removal doesn't end up resetting to 0 all the time
        self.next_paymaster = (self.next_paymaster + 1) % self.paymasters.len();
        let paymaster = self.paymasters.get_mut(next_paymaster);
        paymaster
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

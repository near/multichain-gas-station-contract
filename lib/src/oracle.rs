use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    bs58, ext_contract,
    json_types::{U128, U64},
    serde::{Deserialize, Serialize},
};
use schemars::JsonSchema;

use crate::Rejectable;

pub const PYTH_PRICE_ID_NEAR_USD: &str = "3gnSbT7bhoTdGkFVZc1dW1PvjreWzpUNUD5ppXwv1N59";
pub const PYTH_PRICE_ID_ETH_USD: &str = "EdVCmQ9FSPcVe5YySXDPCRmc8aDQLKJ9xvYBMZPie1Vw";

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, JsonSchema, Clone, Copy)]
#[serde(crate = "near_sdk::serde")]
pub struct Price {
    pub multiplier: U128,
    pub decimals: u8,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(crate = "near_sdk::serde")]
pub struct AssetOptionalPrice {
    pub asset_id: String,
    pub price: Option<Price>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(crate = "near_sdk::serde")]
pub struct PriceData {
    pub timestamp: U64,
    pub recency_duration_sec: u32,

    pub prices: Vec<AssetOptionalPrice>,
}

#[ext_contract(ext_oracle)]
pub trait OracleInterface {
    fn get_price_data(&self, asset_ids: Option<Vec<String>>) -> PriceData;
}

pub fn decode_pyth_price_id(s: &str) -> [u8; 32] {
    let mut b = [0u8; 32];
    bs58::decode(s)
        .into(&mut b)
        .expect_or_reject("Failed to decode Pyth price identifier");
    b
}

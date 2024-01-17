use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    ext_contract,
    json_types::{U128, U64},
    serde::{Deserialize, Serialize},
};

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Copy)]
#[serde(crate = "near_sdk::serde")]
pub struct Price {
    pub multiplier: U128,
    pub decimals: u8,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct AssetOptionalPrice {
    pub asset_id: String,
    pub price: Option<Price>,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct PriceData {
    pub timestamp: U64,
    pub recency_duration_sec: u32,

    pub prices: Vec<AssetOptionalPrice>,
}

#[ext_contract(ext_oracle)]
pub trait Oracle {
    fn get_price_data(&self, asset_ids: Option<Vec<String>>) -> PriceData;
}

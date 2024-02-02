use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env, ext_contract,
    json_types::{U128, U64},
    serde::{Deserialize, Serialize},
};
use schemars::JsonSchema;

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
pub trait Oracle {
    fn get_price_data(&self, asset_ids: Option<Vec<String>>) -> PriceData;
}

pub fn process_oracle_result(
    local_asset_id: &str,
    foreign_asset_id: &str,
    price_data: &PriceData,
) -> (u128, u128) {
    let (local_price, foreign_price) = match &price_data.prices[..] {
        [AssetOptionalPrice {
            asset_id: first_asset_id,
            price: Some(first_price),
        }, AssetOptionalPrice {
            asset_id: second_asset_id,
            price: Some(second_price),
        }] if first_asset_id == local_asset_id && second_asset_id == foreign_asset_id => {
            (first_price, second_price)
        }
        _ => env::panic_str("Invalid price data"),
    };

    (
        foreign_price.multiplier.0 * u128::from(local_price.decimals),
        local_price.multiplier.0 * u128::from(foreign_price.decimals),
    )
}

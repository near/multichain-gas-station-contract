use lib::oracle::{AssetOptionalPrice, Oracle, Price, PriceData};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env, near_bindgen,
};

#[derive(BorshSerialize, BorshDeserialize, Default, Debug)]
#[near_bindgen]
struct Contract {}

#[near_bindgen]
impl Oracle for Contract {
    fn get_price_data(&self, asset_ids: Option<Vec<String>>) -> PriceData {
        let _ = asset_ids;

        PriceData {
            prices: vec![
                AssetOptionalPrice {
                    asset_id: "wrap.testnet".to_string(),
                    price: Some(Price {
                        decimals: 28,
                        multiplier: 28770.into(),
                    }),
                },
                AssetOptionalPrice {
                    asset_id: "weth.fakes.testnet".to_string(),
                    price: Some(Price {
                        decimals: 20,
                        multiplier: 226435.into(),
                    }),
                },
            ],
            recency_duration_sec: 90,
            timestamp: env::block_timestamp().into(),
        }
    }
}

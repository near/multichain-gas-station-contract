use lib::oracle::{AssetOptionalPrice, OracleInterface, Price, PriceData};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env, near_bindgen,
};

#[derive(BorshSerialize, BorshDeserialize, Default, Debug)]
#[near_bindgen]
struct Contract {}

#[near_bindgen]
impl OracleInterface for Contract {
    fn get_price_data(&self, asset_ids: Option<Vec<String>>) -> PriceData {
        let [a, b]: [_; 2] = asset_ids.unwrap().try_into().unwrap();

        match (a.as_str(), b.as_str()) {
            ("wrap.testnet", "weth.fakes.testnet") => PriceData {
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
                            multiplier: 226_435.into(),
                        }),
                    },
                ],
                recency_duration_sec: 90,
                timestamp: env::block_timestamp().into(),
            },
            ("local_ft.testnet", "weth.fakes.testnet") => PriceData {
                prices: vec![
                    AssetOptionalPrice {
                        asset_id: "local_ft.testnet".to_string(),
                        price: Some(Price {
                            decimals: 28,
                            multiplier: 28770.into(),
                        }),
                    },
                    AssetOptionalPrice {
                        asset_id: "weth.fakes.testnet".to_string(),
                        price: Some(Price {
                            decimals: 20,
                            multiplier: 226_435.into(),
                        }),
                    },
                ],
                recency_duration_sec: 90,
                timestamp: env::block_timestamp().into(),
            },
            _ => env::panic_str("Unknown asset pair"),
        }
    }
}

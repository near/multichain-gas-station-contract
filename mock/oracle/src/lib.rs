use lib::oracle::{
    decode_pyth_price_id, AssetOptionalPrice, OracleInterface, Price, PriceData,
    PYTH_PRICE_ID_ETH_USD, PYTH_PRICE_ID_NEAR_USD,
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env, near_bindgen,
};

#[derive(BorshSerialize, BorshDeserialize, Default, Debug)]
#[near_bindgen]
pub struct Contract {}

#[near_bindgen]
impl Contract {
    pub fn get_price(
        &self,
        price_identifier: pyth::state::PriceIdentifier,
    ) -> Option<pyth::state::Price> {
        let near_usd = decode_pyth_price_id(PYTH_PRICE_ID_NEAR_USD);
        let eth_usd = decode_pyth_price_id(PYTH_PRICE_ID_ETH_USD);

        if price_identifier.0 == near_usd {
            Some(pyth::state::Price {
                price: 718120242.into(),
                conf: 420242.into(),
                expo: -8,
                publish_time: 1712830518,
            })
        } else if price_identifier.0 == eth_usd {
            Some(pyth::state::Price {
                price: 357262000000.into(),
                conf: 135000000.into(),
                expo: -8,
                publish_time: 1712830748,
            })
        } else {
            None
        }
    }
}

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

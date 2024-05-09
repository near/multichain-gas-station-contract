use lib::{
    oracle::{decode_pyth_price_id, PYTH_PRICE_ID_ETH_USD, PYTH_PRICE_ID_NEAR_USD},
    pyth,
};
use near_sdk::near;

#[derive(Default, Debug)]
#[near(contract_state)]
pub struct Contract {}

#[near]
impl Contract {
    pub fn get_price(&self, price_identifier: pyth::PriceIdentifier) -> Option<pyth::Price> {
        let near_usd = decode_pyth_price_id(PYTH_PRICE_ID_NEAR_USD);
        let eth_usd = decode_pyth_price_id(PYTH_PRICE_ID_ETH_USD);

        if price_identifier.0 == near_usd {
            Some(pyth::Price {
                price: 718_120_242.into(),
                conf: 420_242.into(),
                expo: -8,
                publish_time: 1_712_830_518,
            })
        } else if price_identifier.0 == eth_usd {
            Some(pyth::Price {
                price: 357_262_000_000.into(),
                conf: 135_000_000.into(),
                expo: -8,
                publish_time: 1_712_830_748,
            })
        } else {
            None
        }
    }
}

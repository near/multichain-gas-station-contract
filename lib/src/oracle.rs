use near_sdk::bs58;

use crate::Rejectable;

pub const PYTH_PRICE_ID_NEAR_USD: &str = "3gnSbT7bhoTdGkFVZc1dW1PvjreWzpUNUD5ppXwv1N59";
pub const PYTH_PRICE_ID_ETH_USD: &str = "EdVCmQ9FSPcVe5YySXDPCRmc8aDQLKJ9xvYBMZPie1Vw";

pub fn decode_pyth_price_id(s: &str) -> [u8; 32] {
    let mut b = [0u8; 32];
    bs58::decode(s)
        .into(&mut b)
        .expect_or_reject("Failed to decode Pyth price identifier");
    b
}

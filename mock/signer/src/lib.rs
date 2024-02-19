use near_multichain_gas_station_contract::signer_contract::{MpcSignature, SignerContract};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    near_bindgen, PromiseOrValue, PublicKey,
};

#[derive(BorshSerialize, BorshDeserialize, Default, Debug)]
#[near_bindgen]
struct Contract {}

#[near_bindgen]
impl SignerContract for Contract {
    fn sign(&mut self, payload: [u8; 32], path: &String) -> PromiseOrValue<MpcSignature> {
        let _ = (payload, path);
        PromiseOrValue::Value(MpcSignature::new([0; 32], [0; 32], 0))
    }

    fn public_key(&self) -> PublicKey {
        "secp256k1:37aFybhUHCxRdDkuCcB3yHzxqK7N8EQ745MujyAQohXSsYymVeHzhLxKvZ2qYeRHf3pGFiAsxqFJZjpF9gP2JV5u"
            .parse()
            .unwrap()
    }
}

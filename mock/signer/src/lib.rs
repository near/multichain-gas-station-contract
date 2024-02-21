use lib::{
    kdf,
    signer::{MpcSignature, SignerInterface},
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env, near_bindgen, PromiseOrValue, PublicKey,
};

#[derive(BorshSerialize, BorshDeserialize, Default, Debug)]
#[near_bindgen]
struct Contract {}

#[near_bindgen]
impl SignerInterface for Contract {
    fn sign(&mut self, payload: [u8; 32], path: &String) -> PromiseOrValue<MpcSignature> {
        let predecessor = env::predecessor_account_id();
        let signing_key = kdf::construct_spoof_key(predecessor.as_bytes(), path.as_bytes());
        let (sig, recid) = signing_key.sign_prehash_recoverable(&payload).unwrap();
        PromiseOrValue::Value(MpcSignature::from_ecdsa_signature(sig, recid).unwrap())
    }

    fn public_key(&self) -> PublicKey {
        "secp256k1:37aFybhUHCxRdDkuCcB3yHzxqK7N8EQ745MujyAQohXSsYymVeHzhLxKvZ2qYeRHf3pGFiAsxqFJZjpF9gP2JV5u"
            .parse()
            .unwrap()
    }
}

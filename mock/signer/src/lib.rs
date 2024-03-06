use lib::{
    kdf::sha256,
    signer::{MpcSignature, SignerInterface},
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env, near_bindgen, PromiseOrValue, PublicKey,
};

pub fn construct_spoof_key(
    predecessor: &[u8],
    path: &[u8],
) -> ethers_core::k256::ecdsa::SigningKey {
    let predecessor_hash = sha256([predecessor, b",", path].concat().as_slice());
    ethers_core::k256::ecdsa::SigningKey::from_bytes(predecessor_hash.as_slice().into()).unwrap()
}

#[derive(BorshSerialize, BorshDeserialize, Default, Debug)]
#[near_bindgen]
struct Contract {}

#[near_bindgen]
impl SignerInterface for Contract {
    fn sign(&mut self, payload: [u8; 32], path: &String) -> PromiseOrValue<MpcSignature> {
        let predecessor = env::predecessor_account_id();
        let signing_key = construct_spoof_key(predecessor.as_bytes(), path.as_bytes());
        let (sig, recid) = signing_key.sign_prehash_recoverable(&payload).unwrap();
        PromiseOrValue::Value(MpcSignature::from_ecdsa_signature(sig, recid).unwrap())
    }

    fn public_key(&self) -> PublicKey {
        "secp256k1:37aFybhUHCxRdDkuCcB3yHzxqK7N8EQ745MujyAQohXSsYymVeHzhLxKvZ2qYeRHf3pGFiAsxqFJZjpF9gP2JV5u"
            .parse()
            .unwrap()
    }
}

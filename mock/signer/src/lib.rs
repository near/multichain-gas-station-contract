use lib::{
    chain_key::*,
    kdf::sha256,
    signer::{MpcSignature, SignerInterface},
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env, near_bindgen, AccountId, PromiseOrValue, PublicKey,
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

#[near_bindgen]
impl ChainKeySign for Contract {
    fn ck_scheme_oid(&self) -> String {
        "1.3.132.0.10".to_string()
    }

    fn ck_sign_hash(
        &mut self,
        owner_id: Option<AccountId>,
        path: String,
        payload: Vec<u8>,
    ) -> PromiseOrValue<ChainKeySignature> {
        let owner_id = owner_id
            .unwrap_or_else(env::predecessor_account_id)
            .as_bytes()
            .to_vec();

        let signing_key = construct_spoof_key(&owner_id, path.as_bytes());
        let (sig, recid) = signing_key.sign_prehash_recoverable(&payload).unwrap();

        PromiseOrValue::Value(
            ethers_core::types::Signature {
                r: sig.r().to_bytes().as_slice().into(),
                s: sig.s().to_bytes().as_slice().into(),
                v: recid.to_byte().into(),
            }
            .to_string(),
        )
    }
}

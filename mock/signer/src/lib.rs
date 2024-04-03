use lib::{
    chain_key::*,
    kdf::sha256,
    signer::{MpcSignature, SignerInterface},
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env, near_bindgen, require, AccountId, PromiseOrValue, PublicKey,
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
pub struct MockSignerContract {}

#[near_bindgen]
impl MockSignerContract {
    pub fn public_key_for(&self, account_id: AccountId, path: String) -> String {
        let signing_key = construct_spoof_key(account_id.as_bytes(), path.as_bytes());
        let verifying_key = signing_key.verifying_key();
        let encoded = verifying_key.to_encoded_point(false);
        encoded.to_string()
    }
}

#[near_bindgen]
impl SignerInterface for MockSignerContract {
    fn sign(
        &mut self,
        payload: [u8; 32],
        path: &String,
        key_version: u32,
    ) -> PromiseOrValue<MpcSignature> {
        require!(key_version == 0, "Key version not supported");
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

    fn latest_key_version(&self) -> u32 {
        0
    }
}

#[near_bindgen]
impl ChainKeySign for MockSignerContract {
    fn ck_scheme_oid(&self) -> String {
        "1.3.132.0.10".to_string()
    }

    fn ck_sign_hash(
        &mut self,
        path: String,
        payload: Vec<u8>,
    ) -> PromiseOrValue<ChainKeySignature> {
        let signing_key =
            construct_spoof_key(env::predecessor_account_id().as_bytes(), path.as_bytes());
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

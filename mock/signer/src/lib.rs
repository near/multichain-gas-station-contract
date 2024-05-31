use lib::{
    kdf::sha256,
    signer::{MpcSignature, SignerInterface},
};
use near_sdk::{env, near, require, AccountId, PromiseOrValue, PublicKey};

#[must_use]
pub fn construct_spoof_key(
    predecessor: &[u8],
    path: &[u8],
) -> ethers_core::k256::ecdsa::SigningKey {
    let predecessor_hash = sha256([predecessor, b",", path].concat().as_slice());
    ethers_core::k256::ecdsa::SigningKey::from_bytes(predecessor_hash.as_slice().into()).unwrap()
}

#[derive(Default, Debug)]
#[near(contract_state)]
pub struct MockSignerContract {}

#[near]
impl MockSignerContract {
    #[must_use]
    pub fn public_key_for(&self, account_id: AccountId, path: String) -> String {
        let signing_key = construct_spoof_key(account_id.as_bytes(), path.as_bytes());
        let verifying_key = signing_key.verifying_key();
        let encoded = verifying_key.to_encoded_point(false);
        encoded.to_string()
    }
}

#[near]
impl SignerInterface for MockSignerContract {
    #[payable]
    fn sign(&mut self, request: lib::signer::SignRequest) -> PromiseOrValue<MpcSignature> {
        require!(request.key_version == 0, "Key version not supported");
        let predecessor = env::predecessor_account_id();
        // This is unused, but needs to be in the sign signature.
        let signing_key = construct_spoof_key(predecessor.as_bytes(), request.path.as_bytes());
        let (sig, recid) = signing_key
            .sign_prehash_recoverable(&request.payload)
            .unwrap();
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

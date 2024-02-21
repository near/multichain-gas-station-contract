use std::str::FromStr;

use lib::kdf;
use lib::signer::{MpcSignature, SignerInterface};
use near_sdk::{
    env,
    json_types::{Base64VecU8, U64},
    near_bindgen,
    serde::{Deserialize, Serialize},
    store::{UnorderedMap, UnorderedSet},
    AccountId, PromiseOrValue,
};
use near_sdk_contract_tools::owner::Owner;

use crate::{Contract, ContractExt, Flags, StorageKey, DEFAULT_EXPIRE_SEQUENCE_IN_NS};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct StorageEntry {
    pub key: Base64VecU8,
    pub value: Base64VecU8,
}

#[near_bindgen]
impl Contract {
    pub fn clear_storage(&mut self, entries: Vec<StorageEntry>) {
        for entry in entries {
            env::storage_remove(&entry.key.0);
        }
    }

    #[init(ignore_state)]
    pub fn new_debug(
        signer_contract_id: AccountId,
        oracle_id: AccountId,
        oracle_local_asset_id: String,
        expire_sequence_after_ns: Option<U64>,
    ) -> Self {
        let mut contract = Self {
            next_unique_id: 0,
            signer_contract_id,
            signer_contract_public_key: None, // Loaded asynchronously
            oracle_id,
            oracle_local_asset_id,
            flags: Flags::default(),
            expire_sequence_after_ns: expire_sequence_after_ns
                .map_or(DEFAULT_EXPIRE_SEQUENCE_IN_NS, u64::from),
            foreign_chains: UnorderedMap::new(StorageKey::ForeignChains),
            sender_whitelist: UnorderedSet::new(StorageKey::SenderWhitelist),
            receiver_whitelist: UnorderedSet::new(StorageKey::ReceiverWhitelist),
            pending_transaction_sequences: UnorderedMap::new(StorageKey::PendingTransactions),
            collected_fees: UnorderedMap::new(StorageKey::CollectedFees),
        };

        Owner::update_owner(&mut contract, Some(env::predecessor_account_id()));

        contract
    }
}

#[near_bindgen]
impl SignerInterface for Contract {
    fn public_key(&self) -> near_sdk::PublicKey {
        near_sdk::PublicKey::from_str("secp256k1:4HFcTSodRLVCGNVcGc4Mf2fwBBBxv9jxkGdiW2S2CA1y6UpVVRWKj6RX7d7TDt65k2Bj3w9FU4BGtt43ZvuhCnNt").unwrap()
    }

    fn sign(&mut self, payload: [u8; 32], path: &String) -> PromiseOrValue<MpcSignature> {
        let predecessor = env::predecessor_account_id();
        let signing_key = kdf::construct_spoof_key(predecessor.as_bytes(), path.as_bytes());
        let (sig, recid) = signing_key.sign_prehash_recoverable(&payload).unwrap();
        PromiseOrValue::Value(MpcSignature::from_ecdsa_signature(sig, recid).unwrap())
    }
}

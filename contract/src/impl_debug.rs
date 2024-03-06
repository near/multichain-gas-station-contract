use near_sdk::{
    env,
    json_types::{Base64VecU8, U64},
    near_bindgen,
    serde::{Deserialize, Serialize},
    store::{UnorderedMap, UnorderedSet, Vector},
    AccountId,
};
use near_sdk_contract_tools::owner::Owner;

use crate::{Contract, ContractExt, Flags, StorageKey, DEFAULT_EXPIRE_SEQUENCE_AFTER_BLOCKS};

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
        expire_sequence_after_blocks: Option<U64>,
    ) -> Self {
        let mut contract = Self {
            next_unique_id: 0,
            signer_contract_id,
            signer_contract_public_key: None, // Loaded asynchronously
            oracle_id,
            oracle_local_asset_id,
            flags: Flags::default(),
            expire_sequence_after_blocks: expire_sequence_after_blocks
                .map_or(DEFAULT_EXPIRE_SEQUENCE_AFTER_BLOCKS, u64::from),
            foreign_chains: UnorderedMap::new(StorageKey::ForeignChains),
            sender_whitelist: UnorderedSet::new(StorageKey::SenderWhitelist),
            receiver_whitelist: UnorderedSet::new(StorageKey::ReceiverWhitelist),
            pending_transaction_sequences: UnorderedMap::new(
                StorageKey::PendingTransactionSequences,
            ),
            signed_transaction_sequences: Vector::new(StorageKey::SignedTransactionSequences),
            collected_fees: UnorderedMap::new(StorageKey::CollectedFees),
        };

        Owner::update_owner(&mut contract, Some(env::predecessor_account_id()));

        contract
    }
}

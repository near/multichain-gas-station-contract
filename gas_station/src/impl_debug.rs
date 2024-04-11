use near_sdk::{
    collections::{UnorderedMap, UnorderedSet, Vector},
    env,
    json_types::{Base64VecU8, U64},
    near_bindgen,
    serde::{Deserialize, Serialize},
    AccountId,
};
use near_sdk_contract_tools::owner::Owner;

use crate::{Contract, ContractExt, Flags, StorageKey, DEFAULT_EXPIRE_SEQUENCE_AFTER_BLOCKS};
use lib::{asset::AssetId, oracle::decode_pyth_price_id};

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
        supported_assets_oracle_asset_ids: Vec<(AssetId, String)>,
        expire_sequence_after_blocks: Option<U64>,
    ) -> Self {
        let mut contract = Self {
            next_unique_id: 0,
            signer_contract_id,
            oracle_id,
            supported_assets_oracle_asset_ids: UnorderedMap::new(StorageKey::SupportedAssets),
            flags: Flags::default(),
            expire_sequence_after_blocks: expire_sequence_after_blocks
                .map_or(DEFAULT_EXPIRE_SEQUENCE_AFTER_BLOCKS, u64::from),
            foreign_chains: UnorderedMap::new(StorageKey::ForeignChains),
            user_chain_keys: UnorderedMap::new(StorageKey::UserChainKeys),
            paymaster_keys: UnorderedMap::new(StorageKey::PaymasterKeys),
            sender_whitelist: UnorderedSet::new(StorageKey::SenderWhitelist),
            receiver_whitelist: UnorderedSet::new(StorageKey::ReceiverWhitelist),
            pending_transaction_sequences: UnorderedMap::new(
                StorageKey::PendingTransactionSequences,
            ),
            signed_transaction_sequences: Vector::new(StorageKey::SignedTransactionSequences),
            collected_fees: UnorderedMap::new(StorageKey::CollectedFees),
        };

        contract.supported_assets_oracle_asset_ids.extend(
            supported_assets_oracle_asset_ids
                .into_iter()
                .map(|(a, s)| (a, decode_pyth_price_id(&s))),
        );

        Owner::update_owner(&mut contract, Some(env::predecessor_account_id()));

        contract
    }
}

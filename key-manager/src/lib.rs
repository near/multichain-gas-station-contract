use interface::{ChainKeyId, ChainKeyManager, ChainKeyPublicKey, ChainKeySignature};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env, near_bindgen,
    store::{LookupMap, UnorderedMap},
    AccountId, BorshStorageKey, PanicOnDefault,
};

mod interface;

#[derive(Debug, Clone, BorshSerialize, BorshStorageKey)]
enum StorageKey {
    KeyGovernor,
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyIdentifier {
    owner_id: AccountId,
    path: String,
}

#[derive(Debug, BorshSerialize, BorshDeserialize, PanicOnDefault)]
#[near_bindgen]
pub struct ManagerContract {
    pub signer_id: AccountId,
    pub key_governor: UnorderedMap<KeyIdentifier, AccountId>,
}

#[near_bindgen]
impl ManagerContract {
    #[init]
    pub fn new(signer_id: AccountId) -> Self {
        Self {
            signer_id,
            key_governor: UnorderedMap::new(StorageKey::KeyGovernor),
        }
    }
}

impl ChainKeyManager for ManagerContract {
    fn ck_get_governor_for_key(&self, owner_id: AccountId, path: String) -> Option<AccountId> {
        self.key_governor
            .get(&KeyIdentifier { owner_id, path })
            .cloned()
    }

    fn ck_transfer_governorship(&mut self, path: String, governor_id: Option<AccountId>) {
        todo!()
    }

    fn ck_resolve_transfer_governorship(&mut self) {
        todo!()
    }

    fn ck_sign(
        &mut self,
        owner_id: Option<AccountId>,
        path: String,
        payload: Vec<u8>,
    ) -> ChainKeySignature {
        todo!()
    }
}

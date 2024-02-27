use lib::chain_key::{ext_chain_key_manager, ChainKeyGovernor, ChainKeySignature};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env, near_bindgen, require,
    store::UnorderedSet,
    AccountId, BorshStorageKey, PanicOnDefault, Promise, PromiseOrValue,
};

#[derive(BorshSerialize, BorshStorageKey)]
enum StorageKey {
    GoverningKeys,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, PanicOnDefault)]
#[near_bindgen]
pub struct GovernorContract {
    pub manager_id: AccountId,
    pub governing_keys: UnorderedSet<(AccountId, String)>,
}

#[near_bindgen]
impl GovernorContract {
    #[init]
    pub fn new(manager_id: AccountId) -> Self {
        Self {
            manager_id,
            governing_keys: UnorderedSet::new(StorageKey::GoverningKeys),
        }
    }

    pub fn sign(&mut self, path: String, payload: Vec<u8>) -> PromiseOrValue<ChainKeySignature> {
        let owner_id = env::predecessor_account_id();

        let item = (owner_id, path);

        require!(self.governing_keys.contains(&item), "Not governed",);

        let (owner_id, path) = item;

        // arbitrary payload filtering here
        require!(payload[0] == 0xff, "Invalid payload");

        PromiseOrValue::Promise(
            ext_chain_key_manager::ext(self.manager_id.clone()).ck_sign_prehashed(
                Some(owner_id),
                path,
                payload,
            ),
        )
    }

    pub fn release_key(&mut self, path: String) -> Promise {
        let owner_id = env::predecessor_account_id();

        let item = (owner_id, path);
        self.governing_keys.remove(&item);
        let (owner_id, path) = item;

        ext_chain_key_manager::ext(self.manager_id.clone()).ck_transfer_governorship(
            Some(owner_id),
            path,
            None,
        )
    }
}

#[near_bindgen]
impl ChainKeyGovernor for GovernorContract {
    fn ck_accept_governorship(&mut self, owner_id: AccountId, path: String) -> bool {
        if env::predecessor_account_id() == self.manager_id {
            self.governing_keys.insert((owner_id, path));
            true
        } else {
            false
        }
    }
}

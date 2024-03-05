use lib::chain_key::{ext_chain_key_manager, ChainKeyApproved, ChainKeySignature};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env, near_bindgen, require,
    store::UnorderedSet,
    AccountId, BorshStorageKey, PanicOnDefault, PromiseOrValue,
};

#[derive(BorshSerialize, BorshStorageKey)]
enum StorageKey {
    GoverningKeys,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, PanicOnDefault)]
#[near_bindgen]
pub struct ApprovedContract {
    pub manager_id: AccountId,
    pub delegated_keys: UnorderedSet<(AccountId, String)>,
}

#[near_bindgen]
impl ApprovedContract {
    #[init]
    pub fn new(manager_id: AccountId) -> Self {
        Self {
            manager_id,
            delegated_keys: UnorderedSet::new(StorageKey::GoverningKeys),
        }
    }

    pub fn sign(&mut self, path: String, payload: Vec<u8>) -> PromiseOrValue<ChainKeySignature> {
        let owner_id = env::predecessor_account_id();

        let item = (owner_id, path);

        require!(self.delegated_keys.contains(&item), "Not delegated");

        let (owner_id, path) = item;

        // arbitrary payload filtering here
        require!(payload[0] == 0xff, "Invalid payload");

        PromiseOrValue::Promise(
            ext_chain_key_manager::ext(self.manager_id.clone()).ck_sign_hash(
                Some(owner_id),
                path,
                payload,
            ),
        )
    }
}

#[near_bindgen]
impl ChainKeyApproved for ApprovedContract {
    fn ck_on_approved(&mut self, owner_id: AccountId, path: String, msg: String) {
        require!(
            env::predecessor_account_id() == self.manager_id,
            "Unknown caller",
        );

        let _ = msg;

        self.delegated_keys.insert((owner_id, path));
    }

    fn ck_on_revoked(&mut self, owner_id: AccountId, path: String, msg: String) {
        require!(
            env::predecessor_account_id() == self.manager_id,
            "Unknown caller",
        );

        let _ = msg;

        self.delegated_keys.remove(&(owner_id, path));
    }
}

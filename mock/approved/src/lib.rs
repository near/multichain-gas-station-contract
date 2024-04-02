use lib::{
    chain_key::{ext_chain_key_sign, ChainKeySignature, ChainKeyTokenApproved},
    Rejectable,
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::UnorderedMap,
    env, near_bindgen, require, AccountId, BorshStorageKey, PanicOnDefault, PromiseOrValue,
};

#[derive(BorshSerialize, BorshStorageKey)]
enum StorageKey {
    GoverningKeys,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, PanicOnDefault)]
#[near_bindgen]
pub struct ApprovedContract {
    pub manager_id: AccountId,
    pub delegated_keys: UnorderedMap<String, (AccountId, u32)>,
}

#[near_bindgen]
impl ApprovedContract {
    #[init]
    pub fn new(manager_id: AccountId) -> Self {
        Self {
            manager_id,
            delegated_keys: UnorderedMap::new(StorageKey::GoverningKeys),
        }
    }

    pub fn sign(&mut self, path: String, payload: Vec<u8>) -> PromiseOrValue<ChainKeySignature> {
        let (account_id, approval_id) = self
            .delegated_keys
            .get(&path)
            .expect_or_reject("Not delegated");

        require!(env::predecessor_account_id() == account_id, "Unauthorized");

        // Arbitrary payload filtering here
        // require!(payload[0] == 0xff, "Invalid payload");

        PromiseOrValue::Promise(
            ext_chain_key_sign::ext(self.manager_id.clone()).ck_sign_hash(
                path,
                payload,
                Some(approval_id),
            ),
        )
    }
}

#[near_bindgen]
impl ChainKeyTokenApproved for ApprovedContract {
    fn ckt_on_approved(
        &mut self,
        owner_id: AccountId,
        path: String,
        approval_id: u32,
        msg: String,
    ) {
        let _ = msg;

        require!(
            env::predecessor_account_id() == self.manager_id,
            "Unknown caller",
        );

        self.delegated_keys.insert(&path, &(owner_id, approval_id));
    }

    fn ckt_on_revoked(&mut self, owner_id: AccountId, path: String, approval_id: u32, msg: String) {
        let _ = (owner_id, approval_id, msg);

        require!(
            env::predecessor_account_id() == self.manager_id,
            "Unknown caller",
        );

        self.delegated_keys.remove(&path);
    }
}

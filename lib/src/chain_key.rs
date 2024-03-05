use near_sdk::{ext_contract, AccountId, PromiseOrValue};

pub type ChainKeySignature = String;

#[ext_contract(ext_chain_key_manager)]
pub trait ChainKeyManager {
    fn ck_scheme_oid(&self) -> String;
    fn ck_approve(
        &mut self,
        path: String,
        account_id: AccountId,
        msg: Option<String>,
    ) -> PromiseOrValue<()>;
    fn ck_revoke(
        &mut self,
        path: String,
        account_id: AccountId,
        msg: Option<String>,
    ) -> PromiseOrValue<()>;
    fn ck_revoke_all(&mut self, path: String) -> u32;
    fn ck_is_approved(&self, owner_id: AccountId, path: String, account_id: AccountId) -> bool;
    fn ck_sign_hash(
        &mut self,
        owner_id: Option<AccountId>,
        path: String,
        payload: Vec<u8>,
    ) -> PromiseOrValue<ChainKeySignature>;
}

#[ext_contract(ext_chain_key_approved)]
pub trait ChainKeyApproved {
    fn ck_on_approved(&mut self, owner_id: AccountId, path: String, msg: String);
    fn ck_on_revoked(&mut self, owner_id: AccountId, path: String, msg: String);
}

use near_sdk::{ext_contract, AccountId, PromiseOrValue};

pub type ChainKeySignature = String;

#[ext_contract(ext_chain_key_manager)]
pub trait ChainKeyManager {
    fn ck_scheme_oid(&self) -> String;
    fn ck_get_governor_for_key(&self, owner_id: AccountId, path: String) -> Option<AccountId>;
    fn ck_transfer_governorship(
        &mut self,
        path: String,
        new_governor_id: Option<AccountId>,
    ) -> PromiseOrValue<()>;
    fn ck_sign_prehashed(
        &mut self,
        owner_id: Option<AccountId>,
        path: String,
        payload: Vec<u8>,
    ) -> PromiseOrValue<ChainKeySignature>;
}

#[ext_contract(ext_chain_key_governor)]
pub trait ChainKeyGovernor {
    fn ck_on_transfer_governorship(
        &mut self,
        owner_id: AccountId,
        path: String,
        new_governor_id: Option<AccountId>,
    ) -> bool;
}

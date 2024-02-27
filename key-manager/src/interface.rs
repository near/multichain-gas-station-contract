use near_sdk::{ext_contract, AccountId};

pub type ChainKeyId = String;
pub type ChainKeySignature = String;
pub type ChainKeyPublicKey = String;

#[ext_contract(ext_chain_key_manager)]
pub trait ChainKeyManager {
    fn ck_get_governor_for_key(&self, owner_id: AccountId, path: String) -> Option<AccountId>;
    fn ck_transfer_governorship(&mut self, path: String, governor_id: Option<AccountId>);
    #[private]
    fn ck_resolve_transfer_governorship(&mut self);
    fn ck_sign(
        &mut self,
        owner_id: Option<AccountId>,
        path: String,
        payload: Vec<u8>,
    ) -> ChainKeySignature;
}

#[ext_contract(ext_chain_key_governor)]
pub trait ChainKeyGovernor {
    fn ck_on_transfer_governorship(
        &mut self,
        owner_id: AccountId,
        path: String,
        public_key: ChainKeyPublicKey,
        new_governor_id: Option<AccountId>,
    ) -> bool;
}

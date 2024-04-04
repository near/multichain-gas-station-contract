use near_sdk::{ext_contract, AccountId, PromiseOrValue};

#[ext_contract(ext_chain_key_token)]
pub trait ChainKeyToken {
    fn ckt_scheme_oid(&self) -> String;
    fn ckt_sign_hash(
        &mut self,
        token_id: String,
        path: Option<String>,
        payload: Vec<u8>,
        approval_id: Option<u32>,
    ) -> PromiseOrValue<String>;
    fn ckt_public_key_for(
        &mut self,
        token_id: String,
        path: Option<String>,
    ) -> PromiseOrValue<String>;
}

#[ext_contract(ext_chain_key_token_approval)]
pub trait ChainKeyTokenApproval {
    fn ckt_approve(
        &mut self,
        token_id: String,
        account_id: AccountId,
        msg: Option<String>,
    ) -> PromiseOrValue<Option<u32>>;
    fn ckt_revoke(
        &mut self,
        token_id: String,
        account_id: AccountId,
        msg: Option<String>,
    ) -> PromiseOrValue<()>;
    fn ckt_revoke_all(&mut self, token_id: String) -> near_sdk::json_types::U64;
    fn ckt_approval_id_for(&self, token_id: String, account_id: AccountId) -> Option<u32>;
}

#[ext_contract(ext_chain_key_token_approval_receiver)]
pub trait ChainKeyTokenApprovalReceiver {
    fn ckt_on_approved(
        &mut self,
        approver_id: AccountId,
        token_id: String,
        approval_id: u32,
        msg: String,
    ) -> PromiseOrValue<()>;
    fn ckt_on_revoked(
        &mut self,
        approver_id: AccountId,
        token_id: String,
        approval_id: u32,
        msg: String,
    ) -> PromiseOrValue<()>;
}

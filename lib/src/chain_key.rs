use near_sdk::{ext_contract, AccountId, PromiseOrValue};

pub type ChainKeySignature = String;

#[ext_contract(ext_chain_key_sign)]
pub trait ChainKeySign {
    fn ck_scheme_oid(&self) -> String;
    fn ck_sign_hash(
        &mut self,
        path: String,
        payload: Vec<u8>, // TODO: choose encoding...base64? Or just accept a String?
                          // TODO: There may be a need for a field like this, e.g. to prove knowledge of a hash preimage.
                          // proof: Option<Vec<u8>>,
                          // approval_id: Option<u32>,
    ) -> PromiseOrValue<ChainKeySignature>;

    // TODO: Should only one sign function exist, or both prehashed and unhashed versions should be required?
    // fn ck_sign(
    //     &mut self,
    //     owner_id: Option<AccountId>,
    //     path: String,
    //     payload: Vec<u8>,
    //     digest: String, // TODO: Is this necessary? It seems like Ethereum uses secp256k1/keccak256 whereas everyone else uses secp256k1/sha256.
    // ) -> PromiseOrValue<ChainKeySignature>;

    // TODO: functions that: verify signature, derive public key?
}

#[ext_contract(ext_chain_key_token_sign)]
pub trait ChainKeyTokenSign {
    fn ckt_scheme_oid(&self) -> String;
    fn ckt_sign_hash(
        &mut self,
        token_id: String,
        path: Option<String>,
        payload: Vec<u8>,
        approval_id: Option<u32>,
    ) -> PromiseOrValue<ChainKeySignature>;
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

#[ext_contract(ext_chain_key_token_approved)]
pub trait ChainKeyTokenApproved {
    fn ckt_on_approved(
        &mut self,
        approver_id: AccountId,
        token_id: String,
        approval_id: u32,
        msg: String,
    );
    fn ckt_on_revoked(
        &mut self,
        approver_id: AccountId,
        token_id: String,
        approval_id: u32,
        msg: String,
    );
}

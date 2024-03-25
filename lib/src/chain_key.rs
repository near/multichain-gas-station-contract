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
        approval_id: Option<u32>,
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

#[ext_contract(ext_chain_key_approval)]
pub trait ChainKeyApproval {
    fn ck_approve(
        &mut self,
        path: String,
        account_id: AccountId,
        msg: Option<String>,
    ) -> PromiseOrValue<Option<u32>>;
    fn ck_revoke(
        &mut self,
        path: String,
        account_id: AccountId,
        msg: Option<String>,
    ) -> PromiseOrValue<()>;
    fn ck_revoke_all(&mut self, path: String) -> u32;
    fn ck_approval_id_for(&self, path: String, account_id: AccountId) -> Option<u32>;
}

#[ext_contract(ext_chain_key_approved)]
pub trait ChainKeyApproved {
    fn ck_on_approved(
        &mut self,
        approver_id: AccountId,
        path: String,
        approval_id: u32,
        msg: String,
    );
    fn ck_on_revoked(
        &mut self,
        approver_id: AccountId,
        path: String,
        approval_id: u32,
        msg: String,
    );
}

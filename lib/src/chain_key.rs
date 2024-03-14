use near_sdk::{ext_contract, AccountId, PromiseOrValue};

pub type ChainKeySignature = String;

#[ext_contract(ext_chain_key_sign)]
pub trait ChainKeySign {
    fn ck_scheme_oid(&self) -> String;
    fn ck_sign_hash(
        &mut self,
        owner_id: Option<AccountId>,
        path: String,
        payload: Vec<u8>, // TODO: choose encoding...base64? Or just accept a String?
        // TODO: There may be a need for a field like this, to prove knowledge of a hash preimage.
        // proof: Option<Vec<u8>>,
    ) -> PromiseOrValue<ChainKeySignature>;

    // TODO: Should only one sign function exist, or both prehashed and unhashed versions should be required?
    // fn ck_sign(
    //     &mut self,
    //     owner_id: Option<AccountId>,
    //     path: String,
    //     payload: Vec<u8>,
    //     digest: String, // TODO: Is this necessary? It seems like Ethereum uses secp256k1/keccak256 whereas everyone else uses secp256k1/sha256.
    // ) -> PromiseOrValue<ChainKeySignature>;
}

#[ext_contract(ext_chain_key_approval)]
pub trait ChainKeyApproval {
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
}

#[ext_contract(ext_chain_key_approved)]
pub trait ChainKeyApproved {
    fn ck_on_approved(&mut self, owner_id: AccountId, path: String, msg: String);
    fn ck_on_revoked(&mut self, owner_id: AccountId, path: String, msg: String);
}

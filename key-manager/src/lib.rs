use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    ext_contract, near_bindgen, AccountId,
};

#[derive(Debug, BorshSerialize, BorshDeserialize)]
#[near_bindgen]
pub struct Contract {}

#[ext_contract(ext_chain_key_manager)]
pub trait ChainKeyManager {
    fn create_key(&mut self, signer_id: AccountId, path: String);
}

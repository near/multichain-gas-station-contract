use near_sdk::{ext_contract, Promise};

#[ext_contract(ext_signer)]
pub trait SignerContract {
    fn sign(&mut self, payload: [u8; 32], path: String) -> Promise;
}

use lib::Rejectable;
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env,
    json_types::U128,
    near_bindgen, PanicOnDefault,
};
#[allow(clippy::wildcard_imports)]
use near_sdk_contract_tools::ft::*;

#[derive(BorshSerialize, BorshDeserialize, Debug, PanicOnDefault, Nep141)]
#[near_bindgen]
pub struct LocalFtContract {}

#[near_bindgen]
impl LocalFtContract {
    #[init]
    pub fn new() -> Self {
        Self {}
    }

    pub fn mint(&mut self, amount: U128) {
        Nep141Controller::mint(
            self,
            &Nep141Mint {
                amount: amount.0,
                receiver_id: &env::predecessor_account_id(),
                memo: None,
            },
        )
        .expect_or_reject("Failed to fungible tokens");
    }
}

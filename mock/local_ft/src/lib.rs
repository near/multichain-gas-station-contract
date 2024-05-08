use lib::Rejectable;
use near_sdk::{env, json_types::U128, near, PanicOnDefault};
#[allow(clippy::wildcard_imports)]
use near_sdk_contract_tools::ft::*;

#[derive(Debug, PanicOnDefault, Nep141)]
#[near(contract_state)]
pub struct LocalFtContract {}

#[near]
impl LocalFtContract {
    #[init]
    pub fn new() -> Self {
        Self {}
    }

    pub fn mint(&mut self, amount: U128) {
        Nep141Controller::mint(
            self,
            &Nep141Mint::new(amount.0, env::predecessor_account_id()),
        )
        .expect_or_reject("Failed to fungible tokens");
    }
}

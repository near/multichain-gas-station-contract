use lib::{
    chain_key::{ext_chain_key_sign, ChainKeySign, ChainKeySignature},
    Rejectable,
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env, near_bindgen, require, AccountId, PanicOnDefault, PromiseOrValue,
};
#[allow(clippy::wildcard_imports)]
use near_sdk_contract_tools::nft::*;

#[derive(BorshSerialize, BorshDeserialize, Debug, PanicOnDefault, Nep171)]
#[near_bindgen]
pub struct NftKeyContract {
    pub next_id: u32,
    pub signer_contract_id: AccountId,
}

#[near_bindgen]
impl NftKeyContract {
    #[init]
    pub fn new(signer_contract_id: AccountId) -> Self {
        Self {
            next_id: 0,
            signer_contract_id,
        }
    }

    pub fn mint(&mut self) -> String {
        let id = self.next_id;
        self.next_id += 1;

        let id = id.to_string();

        Nep171Controller::mint(
            self,
            &Nep171Mint {
                token_ids: std::array::from_ref(&id),
                receiver_id: &env::predecessor_account_id(),
                memo: None,
            },
        )
        .expect_or_reject("Failed to mint new key token");

        id
    }
}

#[near_bindgen]
impl ChainKeySign for NftKeyContract {
    fn ck_sign_hash(
        &mut self,
        owner_id: Option<AccountId>,
        path: String,
        payload: Vec<u8>,
    ) -> PromiseOrValue<ChainKeySignature> {
        require!(owner_id.is_none(), "Delegation not supported");

        let expected_owner_id = env::predecessor_account_id();
        let actual_owner_id = self.token_owner(&path);

        require!(
            Some(&expected_owner_id) == actual_owner_id.as_ref(),
            "Unauthorized",
        );

        PromiseOrValue::Promise(
            ext_chain_key_sign::ext(self.signer_contract_id.clone())
                .ck_sign_hash(None, path, payload),
        )
    }

    fn ck_scheme_oid(&self) -> String {
        "1.3.132.0.10".to_string()
    }
}

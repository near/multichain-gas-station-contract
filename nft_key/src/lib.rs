use lib::{
    chain_key::{
        ext_chain_key_token_approved, ChainKeySignature, ChainKeyTokenApproval, ChainKeyTokenSign,
    },
    signer::{ext_signer, MpcSignature},
    Rejectable,
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::UnorderedMap,
    env, near_bindgen, require,
    serde_json::{self, json},
    AccountId, BorshStorageKey, PanicOnDefault, Promise, PromiseError, PromiseOrValue,
};
use near_sdk_contract_tools::hook::Hook;
#[allow(clippy::wildcard_imports)]
use near_sdk_contract_tools::nft::*;

#[derive(Debug, BorshSerialize, BorshStorageKey)]
enum StorageKey {
    KeyData,
    ApprovalsFor(u32),
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct KeyData {
    pub approvals: UnorderedMap<AccountId, u32>,
    pub mpc_key_version: u32,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, PanicOnDefault, NonFungibleToken)]
#[non_fungible_token(transfer_hook = "Self", burn_hook = "Self")]
#[near_bindgen]
pub struct NftKeyContract {
    pub next_id: u32,
    pub signer_contract_id: AccountId,
    pub key_data: UnorderedMap<u32, KeyData>,
}

fn generate_token_metadata(id: u32) -> TokenMetadata {
    TokenMetadata::new().title(format!("Chain Key Token #{id}"))
}

#[near_bindgen]
impl NftKeyContract {
    #[init]
    pub fn new(signer_contract_id: AccountId) -> Self {
        let mut contract = Self {
            next_id: 0,
            signer_contract_id,
            key_data: UnorderedMap::new(StorageKey::KeyData),
        };

        contract.set_contract_metadata(ContractMetadata::new(
            "Chain Key Token".to_string(),
            "CKT".to_string(),
            None,
        ));

        contract
    }

    fn generate_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn mint(&mut self) -> Promise {
        let id = self.generate_id();
        let predecessor = env::predecessor_account_id();

        ext_signer::ext(self.signer_contract_id.clone())
            .latest_key_version()
            .then(Self::ext(env::current_account_id()).mint_callback(id, predecessor))
    }

    #[private]
    pub fn mint_callback(
        &mut self,
        #[serializer(borsh)] id: u32,
        #[serializer(borsh)] predecessor: AccountId,
        #[callback_result] result: Result<u32, PromiseError>,
    ) -> u32 {
        let latest_key_version = result.unwrap();

        self.key_data.insert(
            &id,
            &KeyData {
                mpc_key_version: latest_key_version,
                approvals: UnorderedMap::new(StorageKey::ApprovalsFor(id)),
            },
        );
        self.mint_with_metadata(id.to_string(), predecessor, generate_token_metadata(id))
            .unwrap_or_reject();

        id
    }
}

#[near_bindgen]
impl ChainKeyTokenSign for NftKeyContract {
    fn ckt_sign_hash(
        &mut self,
        token_id: TokenId,
        path: Option<String>,
        payload: Vec<u8>,
        approval_id: Option<u32>,
    ) -> PromiseOrValue<ChainKeySignature> {
        let id = token_id.parse().expect_or_reject("Invalid token ID");
        let path = path.unwrap_or_default();

        let expected_owner_id = env::predecessor_account_id();
        let actual_owner_id = self.token_owner(&token_id.to_string());

        let key_data = self
            .key_data
            .get(&id)
            .expect_or_reject("Missing data for key");

        require!(
            Some(&expected_owner_id) == actual_owner_id.as_ref()
                || key_data
                    .approvals
                    .get(&env::predecessor_account_id())
                    .zip(approval_id)
                    .map_or(false, |(actual, expected)| actual == expected),
            "Unauthorized",
        );

        PromiseOrValue::Promise(
            ext_signer::ext(self.signer_contract_id.clone())
                .with_unused_gas_weight(10)
                .sign(
                    payload.try_into().unwrap(),
                    &format!("{token_id},{path}"),
                    0,
                )
                .then(
                    Self::ext(env::current_account_id())
                        .with_static_gas(near_sdk::Gas::ONE_TERA * 3)
                        .with_unused_gas_weight(1)
                        .ck_sign_hash_callback(),
                ),
        )
    }

    fn ckt_public_key_for(
        &mut self,
        token_id: TokenId,
        path: Option<String>,
    ) -> PromiseOrValue<String> {
        let id: u32 = token_id.parse().expect_or_reject("Invalid token ID");
        let path = path.unwrap_or_default();

        PromiseOrValue::Promise(
            Promise::new(self.signer_contract_id.clone()).function_call(
                "public_key_for".to_string(),
                serde_json::to_vec(&json!({
                    "account_id": env::current_account_id(),
                    "path": format!("{id},{path}"),
                }))
                .unwrap_or_reject(),
                0,
                env::prepaid_gas() / 10,
            ),
        )
    }

    fn ckt_scheme_oid(&self) -> String {
        "1.3.132.0.10".to_string()
    }
}

#[near_bindgen]
impl NftKeyContract {
    #[private]
    pub fn ck_sign_hash_callback(
        &self,
        #[callback_result] result: Result<MpcSignature, PromiseError>,
    ) -> ChainKeySignature {
        let mpc_signature = result.unwrap();
        let ethers_signature: ethers_core::types::Signature =
            mpc_signature.try_into().unwrap_or_reject();
        ethers_signature.to_string()
    }

    #[private]
    pub fn ck_approve_callback(
        &mut self,
        #[serializer(borsh)] token_id: u32,
        #[serializer(borsh)] account_id: AccountId,
        #[serializer(borsh)] approval_id: u32,
        #[callback_result] result: Result<(), PromiseError>,
    ) -> Option<u32> {
        if result.is_ok() {
            Some(approval_id)
        } else {
            let mut key_data = self.key_data.get(&token_id).unwrap_or_reject();
            let ejected_id = key_data.approvals.remove(&account_id);
            self.key_data.insert(&token_id, &key_data);
            require!(ejected_id == Some(approval_id), "Inconsistent approval ID");
            None
        }
    }
}

#[near_bindgen]
impl ChainKeyTokenApproval for NftKeyContract {
    fn ckt_approve(
        &mut self,
        token_id: TokenId,
        account_id: AccountId,
        msg: Option<String>,
    ) -> PromiseOrValue<Option<u32>> {
        let id = token_id.parse().expect_or_reject("Invalid token ID");
        let actual_owner = Nep171Controller::token_owner(self, &token_id);
        let predecessor = env::predecessor_account_id();
        require!(
            actual_owner.as_ref() == Some(&predecessor),
            format!("Unauthorized {actual_owner:?} != {predecessor}")
        );

        let approval_id = self.generate_id();

        let mut key_data = self
            .key_data
            .get(&id)
            .expect_or_reject("Missing data for key");
        key_data.approvals.insert(&account_id, &approval_id);
        self.key_data.insert(&id, &key_data);

        msg.map_or(PromiseOrValue::Value(Some(approval_id)), |msg| {
            PromiseOrValue::Promise(
                ext_chain_key_token_approved::ext(account_id).ckt_on_approved(
                    predecessor,
                    token_id,
                    approval_id,
                    msg,
                ),
            )
        })
    }

    fn ckt_revoke(
        &mut self,
        token_id: TokenId,
        account_id: AccountId,
        msg: Option<String>,
    ) -> PromiseOrValue<()> {
        let id: u32 = token_id.parse().expect_or_reject("Invalid token ID");
        let actual_owner = Nep171Controller::token_owner(self, &token_id);
        let predecessor = env::predecessor_account_id();
        require!(actual_owner.as_ref() == Some(&predecessor), "Unauthorized");

        let Some(mut key_data) = self.key_data.get(&id) else {
            return PromiseOrValue::Value(());
        };

        let Some(revoked_approval_id) = key_data.approvals.remove(&account_id) else {
            return PromiseOrValue::Value(());
        };

        self.key_data.insert(&id, &key_data);

        msg.map_or(PromiseOrValue::Value(()), |msg| {
            PromiseOrValue::Promise(
                ext_chain_key_token_approved::ext(account_id).ckt_on_revoked(
                    predecessor,
                    token_id,
                    revoked_approval_id,
                    msg,
                ),
            )
        })
    }

    fn ckt_revoke_all(&mut self, token_id: TokenId) -> near_sdk::json_types::U64 {
        let id: u32 = token_id.parse().expect_or_reject("Invalid token ID");
        let actual_owner = Nep171Controller::token_owner(self, &token_id);
        let predecessor = env::predecessor_account_id();
        require!(actual_owner.as_ref() == Some(&predecessor), "Unauthorized");

        let Some(mut key_data) = self.key_data.get(&id) else {
            return 0.into();
        };

        let len = key_data.approvals.len();
        key_data.approvals.clear();
        self.key_data.insert(&id, &key_data);

        len.into()
    }

    fn ckt_approval_id_for(&self, token_id: TokenId, account_id: AccountId) -> Option<u32> {
        let id: u32 = token_id.parse().expect_or_reject("Invalid token ID");

        self.key_data
            .get(&id)
            .and_then(|key_data| key_data.approvals.get(&account_id))
    }
}

impl Hook<NftKeyContract, Nep171Transfer<'_>> for NftKeyContract {
    fn hook<R>(
        contract: &mut NftKeyContract,
        transfer: &Nep171Transfer<'_>,
        f: impl FnOnce(&mut NftKeyContract) -> R,
    ) -> R {
        contract.ckt_revoke_all(transfer.token_id.clone());
        f(contract)
    }
}

impl Hook<NftKeyContract, Nep171Burn<'_>> for NftKeyContract {
    fn hook<R>(
        contract: &mut NftKeyContract,
        burn: &Nep171Burn<'_>,
        f: impl FnOnce(&mut NftKeyContract) -> R,
    ) -> R {
        for token_id in burn.token_ids.iter() {
            contract.ckt_revoke_all(token_id.clone());
        }
        f(contract)
    }
}

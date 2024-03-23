use lib::{
    chain_key::{
        ext_chain_key_approved, ext_chain_key_sign, ChainKeyApproval, ChainKeySign,
        ChainKeySignature,
    },
    nft_key::{NftKeyExtraMetadata, NftKeyMinted},
    Rejectable,
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env, near_bindgen, require,
    serde_json::{self, json},
    store::UnorderedMap,
    AccountId, BorshStorageKey, PanicOnDefault, Promise, PromiseError, PromiseOrValue,
};
use near_sdk_contract_tools::hook::Hook;
#[allow(clippy::wildcard_imports)]
use near_sdk_contract_tools::nft::*;

#[derive(Debug, BorshSerialize, BorshStorageKey)]
enum StorageKey {
    Approvals,
    ApprovalsFor(u32),
}

#[derive(BorshSerialize, BorshDeserialize, Debug, PanicOnDefault, NonFungibleToken)]
#[non_fungible_token(transfer_hook = "Self", burn_hook = "Self")]
#[near_bindgen]
pub struct NftKeyContract {
    pub next_id: u32,
    pub signer_contract_id: AccountId,
    pub approvals: UnorderedMap<u32, UnorderedMap<AccountId, u32>>,
}

#[near_bindgen]
impl NftKeyContract {
    #[init]
    pub fn new(signer_contract_id: AccountId) -> Self {
        let mut contract = Self {
            next_id: 0,
            signer_contract_id,
            approvals: UnorderedMap::new(StorageKey::Approvals),
        };

        contract.set_contract_metadata(ContractMetadata::new(
            "Chain Key".to_string(),
            "CK".to_string(),
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
        let id = self.generate_id().to_string();
        let predecessor = env::predecessor_account_id();

        Promise::new(self.signer_contract_id.clone())
            .function_call(
                "public_key_for".to_string(),
                serde_json::to_vec(&json!({
                    "account_id": env::current_account_id(),
                    "path": id,
                }))
                .unwrap_or_reject(),
                0,
                env::prepaid_gas() / 10,
            )
            .then(Self::ext(env::current_account_id()).mint_callback(predecessor, id))
    }

    #[private]
    pub fn mint_callback(
        &mut self,
        #[serializer(borsh)] receiver_id: AccountId,
        #[serializer(borsh)] id: String,
        #[callback_result] result: Result<String, PromiseError>,
    ) -> NftKeyMinted {
        let public_key = result.unwrap();

        let metadata = NftKeyExtraMetadata {
            public_key: public_key.clone(),
        };

        self.mint_with_metadata(
            id.clone(),
            receiver_id,
            TokenMetadata::new()
                .title(format!("Chain Key #{id}"))
                .extra(serde_json::to_string(&metadata).unwrap_or_reject()),
        )
        .unwrap_or_reject();

        NftKeyMinted {
            key_path: id,
            public_key,
        }
    }
}

#[near_bindgen]
impl ChainKeySign for NftKeyContract {
    fn ck_sign_hash(
        &mut self,
        path: String,
        payload: Vec<u8>,
        approval_id: Option<u32>,
    ) -> PromiseOrValue<ChainKeySignature> {
        let expected_owner_id = env::predecessor_account_id();
        let actual_owner_id = self.token_owner(&path);

        let is_approved = || {
            self.approvals
                .get(&path.parse().expect_or_reject("Invalid token ID"))
                .and_then(|set| set.get(&expected_owner_id))
                .zip(approval_id)
                .map_or(false, |(id, approval_id)| id == &approval_id)
        };

        require!(
            Some(&expected_owner_id) == actual_owner_id.as_ref() || is_approved(),
            "Unauthorized",
        );

        PromiseOrValue::Promise(
            ext_chain_key_sign::ext(self.signer_contract_id.clone())
                .ck_sign_hash(path, payload, None),
        )
    }

    fn ck_scheme_oid(&self) -> String {
        "1.3.132.0.10".to_string()
    }
}

#[near_bindgen]
impl ChainKeyApproval for NftKeyContract {
    fn ck_approve(
        &mut self,
        path: String,
        account_id: AccountId,
        msg: Option<String>,
    ) -> PromiseOrValue<()> {
        let id: u32 = path.parse().expect_or_reject("Invalid token ID");
        let actual_owner = Nep171Controller::token_owner(self, &path);
        let predecessor = env::predecessor_account_id();
        require!(
            actual_owner.as_ref() == Some(&predecessor),
            format!("Unauthorized {actual_owner:?} != {predecessor}")
        );

        let approval_id = self.generate_id();

        self.approvals
            .entry(id)
            .or_insert_with(|| UnorderedMap::new(StorageKey::ApprovalsFor(id)))
            .insert(account_id.clone(), approval_id);

        msg.map_or(PromiseOrValue::Value(()), |msg| {
            PromiseOrValue::Promise(ext_chain_key_approved::ext(account_id).ck_on_approved(
                predecessor,
                path,
                approval_id,
                msg,
            ))
        })
    }

    fn ck_revoke(
        &mut self,
        path: String,
        account_id: AccountId,
        msg: Option<String>,
    ) -> PromiseOrValue<()> {
        let id: u32 = path.parse().expect_or_reject("Invalid token ID");
        let actual_owner = Nep171Controller::token_owner(self, &path);
        let predecessor = env::predecessor_account_id();
        require!(actual_owner.as_ref() == Some(&predecessor), "Unauthorized");

        let near_sdk::store::unordered_map::Entry::Occupied(mut entry) = self.approvals.entry(id)
        else {
            return PromiseOrValue::Value(());
        };

        let Some(revoked_approval_id) = entry.get_mut().remove(&account_id) else {
            return PromiseOrValue::Value(());
        };

        msg.map_or(PromiseOrValue::Value(()), |msg| {
            PromiseOrValue::Promise(ext_chain_key_approved::ext(account_id).ck_on_revoked(
                predecessor,
                path,
                revoked_approval_id,
                msg,
            ))
        })
    }

    fn ck_revoke_all(&mut self, path: String) -> u32 {
        let id: u32 = path.parse().expect_or_reject("Invalid token ID");
        let actual_owner = Nep171Controller::token_owner(self, &path);
        let predecessor = env::predecessor_account_id();
        require!(actual_owner.as_ref() == Some(&predecessor), "Unauthorized");

        let near_sdk::store::unordered_map::Entry::Occupied(mut entry) = self.approvals.entry(id)
        else {
            return 0;
        };

        let set = entry.get_mut();
        let len = set.len();
        set.clear();

        len
    }

    fn ck_approval_id_for(&self, path: String, account_id: AccountId) -> Option<u32> {
        let id: u32 = path.parse().expect_or_reject("Invalid token ID");

        self.approvals
            .get(&id)
            .and_then(|approvals| approvals.get(&account_id))
            .copied()
    }
}

impl Hook<NftKeyContract, Nep171Transfer<'_>> for NftKeyContract {
    fn hook<R>(
        contract: &mut NftKeyContract,
        transfer: &Nep171Transfer<'_>,
        f: impl FnOnce(&mut NftKeyContract) -> R,
    ) -> R {
        contract.ck_revoke_all(transfer.token_id.clone());
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
            contract.ck_revoke_all(token_id.clone());
        }
        f(contract)
    }
}

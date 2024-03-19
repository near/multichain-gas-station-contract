use lib::{
    chain_key::{
        ext_chain_key_approved, ext_chain_key_sign, ChainKeyApproval, ChainKeySign,
        ChainKeySignature,
    },
    Rejectable,
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env, near_bindgen, require,
    store::{UnorderedMap, UnorderedSet},
    AccountId, BorshStorageKey, PanicOnDefault, PromiseOrValue,
};
use near_sdk_contract_tools::hook::Hook;
#[allow(clippy::wildcard_imports)]
use near_sdk_contract_tools::nft::*;

#[derive(Debug, BorshSerialize, BorshStorageKey)]
enum StorageKey {
    Approvals,
    ApprovalsFor(u32),
}

#[derive(BorshSerialize, BorshDeserialize, Debug, PanicOnDefault, Nep171)]
#[nep171(transfer_hook = "Self", burn_hook = "Self")]
#[near_bindgen]
pub struct NftKeyContract {
    pub next_id: u32,
    pub signer_contract_id: AccountId,
    pub approvals: UnorderedMap<u32, UnorderedSet<AccountId>>,
}

#[near_bindgen]
impl NftKeyContract {
    #[init]
    pub fn new(signer_contract_id: AccountId) -> Self {
        Self {
            next_id: 0,
            signer_contract_id,
            approvals: UnorderedMap::new(StorageKey::Approvals),
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
        path: String,
        payload: Vec<u8>,
    ) -> PromiseOrValue<ChainKeySignature> {
        let expected_owner_id = env::predecessor_account_id();
        let actual_owner_id = self.token_owner(&path);

        let is_approved = || {
            self.approvals
                .get(&path.parse().expect_or_reject("Invalid token ID"))
                .map(|set| set.contains(&expected_owner_id))
                .unwrap_or(false)
        };

        require!(
            Some(&expected_owner_id) == actual_owner_id.as_ref() || is_approved(),
            "Unauthorized",
        );

        PromiseOrValue::Promise(
            ext_chain_key_sign::ext(self.signer_contract_id.clone()).ck_sign_hash(path, payload),
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

        self.approvals
            .entry(id)
            .or_insert_with(|| UnorderedSet::new(StorageKey::ApprovalsFor(id)))
            .insert(account_id.clone());

        msg.map_or(PromiseOrValue::Value(()), |msg| {
            PromiseOrValue::Promise(ext_chain_key_approved::ext(account_id).ck_on_approved(
                predecessor,
                path,
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

        entry.get_mut().remove(&account_id);

        msg.map_or(PromiseOrValue::Value(()), |msg| {
            PromiseOrValue::Promise(ext_chain_key_approved::ext(account_id).ck_on_approved(
                predecessor,
                path,
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

    fn ck_is_approved(&self, path: String, account_id: AccountId) -> bool {
        let id: u32 = path.parse().expect_or_reject("Invalid token ID");

        self.approvals
            .get(&id)
            .map_or(false, |approvals| approvals.contains(&account_id))
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

use lib::{
    chain_key::{ext_chain_key_approved, ChainKeyApproval, ChainKeySign, ChainKeySignature},
    signer::ext_signer,
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env, near_bindgen, require,
    store::{LookupMap, UnorderedSet},
    AccountId, BorshStorageKey, PanicOnDefault, PromiseOrValue,
};

#[derive(Debug, Clone, BorshSerialize, BorshStorageKey)]
enum StorageKey {
    Permissions,
    PermissionFor(KeyId),
}

#[derive(Debug, Clone, BorshSerialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyId(AccountId, String);

#[derive(Debug, BorshSerialize, BorshDeserialize, PanicOnDefault)]
#[near_bindgen]
pub struct ManagerContract {
    pub signer_contract_id: AccountId,
    pub permissions: LookupMap<KeyId, UnorderedSet<AccountId>>,
}

#[near_bindgen]
impl ManagerContract {
    #[init]
    pub fn new(signer_contract_id: AccountId) -> Self {
        Self {
            signer_contract_id,
            permissions: LookupMap::new(StorageKey::Permissions),
        }
    }

    /// Only returns `true` if the account has been assigned permission. Will
    /// return `false` for the owner (unless the owner is also assigned as a
    /// signer).
    fn is_approved(&self, key_id: &KeyId, account_id: &AccountId) -> bool {
        self.permissions
            .get(key_id)
            .map_or(false, |signers| signers.contains(account_id))
    }
}

#[near_bindgen]
impl ChainKeySign for ManagerContract {
    fn ck_scheme_oid(&self) -> String {
        // Secp256k1 -> prehash is 32 bytes
        "1.3.132.0.10".to_string()
    }

    fn ck_sign_hash(
        &mut self,
        owner_id: Option<AccountId>,
        path: String,
        payload: Vec<u8>,
    ) -> PromiseOrValue<ChainKeySignature> {
        let payload: [u8; 32] = payload
            .try_into()
            .unwrap_or_else(|_| env::panic_str("Invalid payload length"));

        let predecessor = env::predecessor_account_id();
        let owner_id = owner_id.unwrap_or_else(|| predecessor.clone());

        let key_id = KeyId(owner_id.clone(), path.clone());

        require!(
            owner_id == env::predecessor_account_id() || self.is_approved(&key_id, &predecessor),
            "Unauthorized",
        );

        PromiseOrValue::Promise(
            ext_signer::ext(self.signer_contract_id.clone())
                .sign(payload, &format!("{}/{}", owner_id, path)),
        )
    }
}

#[near_bindgen]
impl ChainKeyApproval for ManagerContract {
    fn ck_approve(
        &mut self,
        path: String,
        account_id: AccountId,
        msg: Option<String>,
    ) -> PromiseOrValue<()> {
        // As opposed to the NFT approval functions (NEP-178 - https://nomicon.io/Standards/Tokens/NonFungibleToken/ApprovalManagement#what-is-an-approval-id),
        // this standard does _not_ require approval IDs. This is because this
        // standard does not support transfers, so there is no risk of
        // "re-using" approvals from a previous owner.
        let owner_id = env::predecessor_account_id();

        let permissions = self
            .permissions
            .entry(KeyId(owner_id.clone(), path.clone()))
            .or_insert_with(|| {
                UnorderedSet::new(StorageKey::PermissionFor(KeyId(
                    owner_id.clone(),
                    path.clone(),
                )))
            });

        // Because this standard supports non-notifying approvals and
        // revocations, this is a somewhat thin protection, really only
        // avoiding double-notifying when a duplicate call is accidental.
        let did_not_previously_have_permission = permissions.insert(account_id.clone());

        if did_not_previously_have_permission {
            if let Some(msg) = msg {
                return PromiseOrValue::Promise(
                    ext_chain_key_approved::ext(account_id).ck_on_approved(
                        owner_id.clone(),
                        path.clone(),
                        msg,
                    ),
                );
            }
        }

        PromiseOrValue::Value(())
    }

    fn ck_revoke(
        &mut self,
        path: String,
        account_id: AccountId,
        msg: Option<String>,
    ) -> PromiseOrValue<()> {
        let owner_id = env::predecessor_account_id();

        let permissions = if let Some(permissions) = self
            .permissions
            .get_mut(&KeyId(owner_id.clone(), path.clone()))
        {
            permissions
        } else {
            return PromiseOrValue::Value(());
        };

        let account_had_signing_permission = permissions.remove(&account_id);

        if account_had_signing_permission {
            if let Some(msg) = msg {
                return PromiseOrValue::Promise(
                    ext_chain_key_approved::ext(account_id.clone()).ck_on_revoked(
                        owner_id.clone(),
                        path.clone(),
                        msg,
                    ),
                );
            }
        }

        PromiseOrValue::Value(())
    }

    fn ck_revoke_all(&mut self, path: String) -> u32 {
        let owner_id = env::predecessor_account_id();
        let key_id = KeyId(owner_id, path);
        if let Some(permissions) = self.permissions.get_mut(&key_id) {
            let len = permissions.len();
            permissions.clear();
            self.permissions.remove(&key_id);
            len
        } else {
            0
        }
    }

    fn ck_is_approved(&self, owner_id: AccountId, path: String, account_id: AccountId) -> bool {
        self.is_approved(&KeyId(owner_id, path), &account_id)
    }
}

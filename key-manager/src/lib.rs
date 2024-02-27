use lib::{
    chain_key::{ext_chain_key_governor, ChainKeyManager, ChainKeySignature},
    signer::ext_signer,
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env, near_bindgen, require,
    store::{lookup_map::Entry, LookupMap},
    AccountId, BorshStorageKey, PanicOnDefault, PromiseError, PromiseOrValue,
};

#[derive(Debug, Clone, BorshSerialize, BorshStorageKey)]
enum StorageKey {
    KeyGovernor,
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyIdentifier {
    owner_id: AccountId,
    path: String,
}

#[derive(Debug, BorshSerialize, BorshDeserialize, PanicOnDefault)]
#[near_bindgen]
pub struct ManagerContract {
    pub signer_id: AccountId,
    pub key_governor: LookupMap<KeyIdentifier, AccountId>,
}

#[near_bindgen]
impl ManagerContract {
    #[init]
    pub fn new(signer_id: AccountId) -> Self {
        Self {
            signer_id,
            key_governor: LookupMap::new(StorageKey::KeyGovernor),
        }
    }
}

#[near_bindgen]
impl ChainKeyManager for ManagerContract {
    fn ck_scheme_oid(&self) -> String {
        // Secp256k1 -> prehash is 32 bytes
        "1.3.132.0.10".to_string()
    }

    fn ck_get_governor_for_key(&self, owner_id: AccountId, path: String) -> Option<AccountId> {
        self.key_governor
            .get(&KeyIdentifier { owner_id, path })
            .cloned()
    }

    fn ck_transfer_governorship(
        &mut self,
        path: String,
        new_governor_id: Option<AccountId>,
    ) -> PromiseOrValue<()> {
        let owner_id = env::predecessor_account_id();

        match self.key_governor.entry(KeyIdentifier {
            owner_id: owner_id.clone(),
            path: path.clone(),
        }) {
            // external governor is already assigned, so check in with that governor before allowing the transfer
            Entry::Occupied(e) => PromiseOrValue::Promise(
                ext_chain_key_governor::ext(e.get().clone())
                    .ck_on_transfer_governorship(
                        owner_id.clone(),
                        path.clone(),
                        new_governor_id.clone(),
                    )
                    .then(
                        Self::ext(env::current_account_id()).ck_resolve_transfer_governorship(
                            owner_id,
                            path,
                            new_governor_id,
                        ),
                    ),
            ),
            // no external governor is assigned
            Entry::Vacant(e) => {
                if let Some(new_governor_id) = new_governor_id {
                    e.insert(new_governor_id);
                }
                PromiseOrValue::Value(())
            }
        }
    }

    fn ck_sign_prehashed(
        &mut self,
        owner_id: Option<AccountId>,
        path: String,
        payload: Vec<u8>,
    ) -> PromiseOrValue<ChainKeySignature> {
        let payload: [u8; 32] = payload
            .try_into()
            .unwrap_or_else(|_| env::panic_str("Invalid payload length"));

        let owner_id = owner_id.unwrap_or_else(env::predecessor_account_id);
        let governor = self
            .key_governor
            .get(&KeyIdentifier {
                owner_id: owner_id.clone(),
                path: path.clone(),
            })
            .unwrap_or(&owner_id);

        require!(governor == &env::predecessor_account_id(), "Unauthorized");

        PromiseOrValue::Promise(
            ext_signer::ext(self.signer_id.clone())
                .sign(payload, &format!("{}/{}", owner_id, path)),
        )
    }
}

#[near_bindgen]
impl ManagerContract {
    #[private]
    pub fn ck_resolve_transfer_governorship(
        &mut self,
        #[serializer(borsh)] owner_id: AccountId,
        #[serializer(borsh)] path: String,
        #[serializer(borsh)] new_governor_id: Option<AccountId>,
        #[callback_result] result: Result<bool, PromiseError>,
    ) {
        require!(matches!(result, Ok(true)), "Governorship transfer failed");

        let identifier = KeyIdentifier { owner_id, path };

        if let Some(new_governor_id) = new_governor_id {
            self.key_governor.insert(identifier, new_governor_id);
        } else {
            self.key_governor.remove(&identifier);
        }
    }
}

use ethers_core::k256::EncodedPoint;
use lib::{nft_key::NftKeyExtraMetadata, Rejectable};
use near_sdk::{
    borsh, env, near_bindgen, require,
    serde::{Deserialize, Serialize},
    store::UnorderedMap,
    AccountId, Promise, PromiseError, PromiseOrValue,
};
use near_sdk_contract_tools::{nft::*, owner::OwnerExternal};

#[allow(unused_imports)]
use crate::ContractExt;
use crate::{Contract, StorageKey};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(crate = "near_sdk::serde")]
pub struct Nep171ReceiverMsg {
    pub is_paymaster: bool,
}

#[near_bindgen]
impl Nep171Receiver for Contract {
    fn nft_on_transfer(
        &mut self,
        sender_id: AccountId,
        previous_owner_id: AccountId,
        token_id: TokenId,
        msg: String,
    ) -> PromiseOrValue<bool> {
        let predecessor = env::predecessor_account_id();

        require!(
            predecessor == self.signer_contract_id,
            "Unknown NFT contract",
        );

        PromiseOrValue::Promise(
            ext_nep171::ext(env::predecessor_account_id())
                .nft_token(token_id.clone())
                .then(
                    Self::ext(env::current_account_id()).nft_on_transfer_callback(
                        sender_id,
                        previous_owner_id,
                        token_id,
                        msg,
                    ),
                ),
        )
    }
}

fn get_public_key_from_token(token: &Token) -> EncodedPoint {
    let token_metadata: TokenMetadata = near_sdk::serde_json::from_value(
        token
            .extensions_metadata
            .get("metadata")
            .unwrap_or_reject()
            .clone(),
    )
    .unwrap_or_reject();

    let extra_metadata: NftKeyExtraMetadata = near_sdk::serde_json::from_str(
        &token_metadata
            .extra
            .expect_or_reject("Missing extra metadata containing public key"),
    )
    .unwrap_or_reject();

    <EncodedPoint as std::str::FromStr>::from_str(&extra_metadata.public_key).unwrap_or_reject()
}

#[near_bindgen]
impl Contract {
    #[private]
    pub fn nft_on_transfer_callback(
        &mut self,
        #[serializer(borsh)] sender_id: AccountId,
        #[serializer(borsh)] previous_owner_id: AccountId,
        #[serializer(borsh)] token_id: TokenId,
        #[serializer(borsh)] msg: String,
        #[callback_result] result: Result<Token, PromiseError>,
    ) -> PromiseOrValue<bool> {
        let _ = sender_id;

        let public_key = get_public_key_from_token(&result.unwrap());

        let sent_from_contract_owner = self
            .own_get_owner()
            .map_or(false, |owner| owner == previous_owner_id);

        let marked_as_paymaster_key = || {
            near_sdk::serde_json::from_str::<Nep171ReceiverMsg>(&msg)
                .map_or(false, |m| m.is_paymaster)
        };

        if sent_from_contract_owner && marked_as_paymaster_key() {
            self.paymaster_keys
                .insert(token_id, public_key.to_bytes().into_vec());
        } else {
            let user_keys = self
                .user_keys
                .entry(previous_owner_id.clone())
                .or_insert_with(|| {
                    UnorderedMap::new(StorageKey::ManagedKeysFor(previous_owner_id))
                });

            user_keys.insert(token_id, public_key.to_bytes().into_vec());
        }

        PromiseOrValue::Value(false)
    }

    pub fn recover_nft_key(&mut self, token_id: TokenId, msg: Option<String>) -> Promise {
        let predecessor = env::predecessor_account_id();

        let user_keys = self
            .user_keys
            .get_mut(&predecessor)
            .expect_or_reject("No managed keys found for predecessor");

        let owned = user_keys.remove(&token_id);

        require!(
            owned.is_some(),
            "Token was not sent to this contract by predecessor"
        );

        if let Some(msg) = msg {
            ext_nep171::ext(self.signer_contract_id.clone()).nft_transfer_call(
                predecessor,
                token_id,
                None,
                None,
                msg,
            )
        } else {
            ext_nep171::ext(self.signer_contract_id.clone()).nft_transfer(
                predecessor,
                token_id,
                None,
                None,
            )
        }
    }
}

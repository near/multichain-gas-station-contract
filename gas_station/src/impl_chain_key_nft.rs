use ethers_core::k256::EncodedPoint;
use lib::{
    chain_key::{ext_chain_key_token, ChainKeyTokenApprovalReceiver},
    Rejectable,
};
use near_sdk::{
    borsh,
    collections::UnorderedMap,
    env, near_bindgen, require,
    serde::{Deserialize, Serialize},
    AccountId, Promise, PromiseError, PromiseOrValue,
};
use near_sdk_contract_tools::{
    nft::{ext_nep171, Nep171Receiver, TokenId},
    rbac::Rbac,
};

#[allow(unused_imports)]
use crate::ContractExt;
use crate::{ChainKeyAuthorization, ChainKeyData, Contract, Role, StorageKey};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(crate = "near_sdk::serde")]
pub struct ChainKeyReceiverMsg {
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
        self.require_unpaused_or_administrator(&sender_id);

        #[cfg(feature = "debug")]
        {
            let _ = (sender_id, previous_owner_id, token_id, msg);
            env::panic_str("This contract is in debug mode. Use chain key approvals only to prevent loss of NFTs.");
        }

        #[cfg(not(feature = "debug"))]
        {
            let _ = sender_id;

            let predecessor = env::predecessor_account_id();

            require!(
                predecessor == self.signer_contract_id,
                "Unknown chain key NFT contract",
            );

            PromiseOrValue::Promise(
                ext_chain_key_token::ext(env::predecessor_account_id())
                    .ckt_public_key_for(token_id.clone(), None)
                    .then(
                        Self::ext(env::current_account_id()).receive_chain_key_callback(
                            previous_owner_id,
                            token_id,
                            ChainKeyAuthorization::Owned,
                            msg,
                        ),
                    ),
            )
        }
    }
}

#[near_bindgen]
impl Contract {
    #[private]
    pub fn receive_chain_key_callback(
        &mut self,
        #[serializer(borsh)] account_id: AccountId,
        #[serializer(borsh)] token_id: TokenId,
        #[serializer(borsh)] authorization: ChainKeyAuthorization,
        #[serializer(borsh)] msg: String,
        #[callback_result] result: Result<String, PromiseError>,
    ) -> PromiseOrValue<bool> {
        let public_key =
            <EncodedPoint as std::str::FromStr>::from_str(&result.unwrap()).unwrap_or_reject();

        let sent_from_contract_administrator =
            <Self as Rbac>::has_role(&account_id, &Role::Administrator);

        let marked_as_paymaster_key = || {
            near_sdk::serde_json::from_str::<ChainKeyReceiverMsg>(&msg)
                .map_or(false, |m| m.is_paymaster)
        };

        let public_key_bytes = public_key.to_bytes().into_vec();
        let key_data = ChainKeyData {
            public_key_bytes,
            authorization,
        };

        if sent_from_contract_administrator && marked_as_paymaster_key() {
            self.paymaster_keys.insert(&token_id, &key_data);
        } else {
            let mut user_chain_keys = self.user_chain_keys.get(&account_id).unwrap_or_else(|| {
                UnorderedMap::new(StorageKey::UserChainKeysFor(account_id.clone()))
            });

            user_chain_keys.insert(&token_id, &key_data);
            self.user_chain_keys.insert(&account_id, &user_chain_keys);
        }

        PromiseOrValue::Value(false)
    }

    pub fn recover_nft_key(&mut self, token_id: TokenId, msg: Option<String>) -> Promise {
        let predecessor = env::predecessor_account_id();
        self.require_unpaused_or_administrator(&predecessor);

        let mut user_keys = self
            .user_chain_keys
            .get(&predecessor)
            .expect_or_reject("No managed keys found for predecessor");

        let owned = user_keys
            .remove(&token_id)
            .expect_or_reject("Token was not sent to this contract by predecessor");

        self.user_chain_keys.insert(&predecessor, &user_keys);

        require!(
            owned.authorization.is_owned(),
            "The key is not owned by this contract",
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

#[near_bindgen]
impl ChainKeyTokenApprovalReceiver for Contract {
    fn ckt_on_approved(
        &mut self,
        approver_id: AccountId,
        token_id: String,
        approval_id: u32,
        msg: String,
    ) -> PromiseOrValue<bool> {
        self.require_unpaused_or_administrator(&approver_id);

        let predecessor = env::predecessor_account_id();

        require!(
            predecessor == self.signer_contract_id,
            "Unknown chain key NFT contract",
        );

        PromiseOrValue::Promise(
            ext_chain_key_token::ext(predecessor)
                .ckt_public_key_for(token_id.clone(), None)
                .then(
                    Self::ext(env::current_account_id()).receive_chain_key_callback(
                        approver_id,
                        token_id,
                        ChainKeyAuthorization::Approved(approval_id),
                        msg,
                    ),
                ),
        )
    }

    fn ckt_on_revoked(
        &mut self,
        approver_id: AccountId,
        token_id: String,
        approval_id: u32,
        msg: String,
    ) -> PromiseOrValue<()> {
        let _ = approval_id;

        require!(msg.is_empty(), "Unsupported msg value");

        let predecessor = env::predecessor_account_id();

        require!(
            predecessor == self.signer_contract_id,
            "Unknown chain key NFT contract",
        );

        let Some(mut user_chain_keys) = self.user_chain_keys.get(&approver_id) else {
            return PromiseOrValue::Value(());
        };

        let removed = user_chain_keys.remove(&token_id);
        self.user_chain_keys.insert(&approver_id, &user_chain_keys);

        if let Some(removed) = removed {
            require!(
                removed.authorization.is_approved(),
                "Contract does not know any approvals for this key",
            );
        }

        PromiseOrValue::Value(())
    }
}

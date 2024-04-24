use lib::asset::{AssetBalance, AssetId};
use near_sdk::{env, json_types::U128, near_bindgen, AccountId, PromiseOrValue};
use near_sdk_contract_tools::ft::Nep141Receiver;

use crate::{Contract, ContractExt, Nep141ReceiverCreateTransactionArgs};

#[near_bindgen]
impl Nep141Receiver for Contract {
    fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        self.require_unpaused_or_administrator(&sender_id);

        // TODO: Some way to inform the sender_id of the transaction ID that just got created

        let asset_id = AssetId::Nep141(env::predecessor_account_id());

        let asset_is_supported = self.accepted_local_assets.get(&asset_id).is_some();

        if !asset_is_supported {
            // Unknown assets: ignore.
            return PromiseOrValue::Value(0.into());
        }

        let Ok(Nep141ReceiverCreateTransactionArgs {
            token_id,
            transaction_rlp_hex,
            use_paymaster,
        }) = near_sdk::serde_json::from_str::<Nep141ReceiverCreateTransactionArgs>(&msg)
        else {
            return PromiseOrValue::Value(0.into());
        };

        let creation_promise_or_value = self.create_transaction_inner(
            token_id,
            sender_id,
            transaction_rlp_hex,
            use_paymaster,
            AssetBalance { asset_id, amount },
        );

        match creation_promise_or_value {
            PromiseOrValue::Promise(p) => p
                .then(Self::ext(env::current_account_id()).return_zero())
                .into(),
            PromiseOrValue::Value(_v) => PromiseOrValue::Value(U128(0)),
        }
    }
}

#[near_bindgen]
impl Contract {
    #[private]
    pub fn return_zero(&self) -> U128 {
        U128(0)
    }
}

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
        // TODO: Some way to inform the sender_id of the transaction ID that just got created

        let asset_id = AssetId::Nep141(env::predecessor_account_id());

        let asset_is_supported = self
            .supported_assets_oracle_asset_ids
            .contains_key(&asset_id);

        if !asset_is_supported {
            // Unknown assets: ignore.
            return PromiseOrValue::Value(0.into());
        }

        let args = if let Ok(args) =
            near_sdk::serde_json::from_str::<Nep141ReceiverCreateTransactionArgs>(&msg)
        {
            args
        } else {
            return PromiseOrValue::Value(0.into());
        };

        let creation_promise_or_value = self.create_transaction_inner(
            sender_id,
            args.transaction_rlp_hex,
            args.use_paymaster,
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

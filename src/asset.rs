use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    json_types::U128,
    serde::{Deserialize, Serialize},
    AccountId, Promise,
};
use near_sdk_contract_tools::standard::nep141::ext_nep141;
use schemars::JsonSchema;

#[derive(
    Serialize,
    Deserialize,
    BorshSerialize,
    BorshDeserialize,
    JsonSchema,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Clone,
    Debug,
)]
#[serde(crate = "near_sdk::serde")]
pub enum AssetId {
    Native,
    Nep141(AccountId),
}

impl AssetId {
    pub fn transfer(&self, receiver_id: AccountId, amount: impl Into<u128>) -> Promise {
        match self {
            AssetId::Native => Promise::new(receiver_id).transfer(amount.into()),
            AssetId::Nep141(contract_id) => ext_nep141::ext(contract_id.clone()).ft_transfer(
                receiver_id,
                U128(amount.into()),
                None,
            ),
        }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, BorshSerialize, BorshDeserialize, Clone, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct AssetBalance {
    pub asset_id: AssetId,
    pub amount: U128,
}

impl AssetBalance {
    pub fn native(amount: impl Into<U128>) -> Self {
        Self {
            asset_id: AssetId::Native,
            amount: amount.into(),
        }
    }

    pub fn nep141(account_id: AccountId, amount: impl Into<U128>) -> Self {
        Self {
            asset_id: AssetId::Nep141(account_id),
            amount: amount.into(),
        }
    }
}

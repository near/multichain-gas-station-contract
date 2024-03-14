use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    json_types::U64,
    serde::{Deserialize, Serialize},
};
use schemars::JsonSchema;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(crate = "near_sdk::serde")]
pub struct Nep141ReceiverCreateTransactionArgs {
    pub transaction_rlp_hex: String,
    pub use_paymaster: Option<bool>,
}

#[derive(
    Serialize,
    Deserialize,
    BorshSerialize,
    BorshDeserialize,
    JsonSchema,
    Clone,
    Debug,
    PartialEq,
    Eq,
)]
#[serde(crate = "near_sdk::serde")]
pub struct TransactionSequenceCreation {
    pub id: U64,
    pub pending_signature_count: u32,
}

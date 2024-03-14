use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    json_types::U64,
    serde::{Deserialize, Serialize},
    AccountId,
};
use near_sdk_contract_tools::event;

use crate::PendingTransactionSequence;

/// A successful request will emit two events, one for the request and one for
/// the finalized transaction, in that order. The `id` field will be the same
/// for both events.
///
/// IDs are arbitrarily chosen by the contract. An ID is guaranteed to be unique
/// within the contract.
#[event(version = "0.1.0", standard = "x-gas-station")]
pub enum ContractEvent {
    TransactionSequenceCreated(TransactionSequenceCreated),
    TransactionSequenceSigned(TransactionSequenceSigned),
}

#[derive(Serialize, Deserialize, BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq)]
#[serde(crate = "near_sdk::serde")]
pub struct TransactionSequenceCreated {
    pub id: U64,
    pub foreign_chain_id: String,
    pub pending_transaction_sequence: PendingTransactionSequence,
}

#[derive(Serialize, Deserialize, BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq)]
#[serde(crate = "near_sdk::serde")]
pub struct TransactionSequenceSigned {
    pub id: U64,
    pub foreign_chain_id: String,
    pub created_by_account_id: AccountId,
    pub signed_transactions: Vec<String>,
}

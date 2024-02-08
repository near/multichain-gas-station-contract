use near_sdk::AccountId;
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
    TransactionSequenceCreated {
        foreign_chain_id: String,
        pending_transaction_sequence: PendingTransactionSequence,
    },
    TransactionSequenceSigned {
        foreign_chain_id: String,
        sender_local_address: AccountId,
        signed_transactions: Vec<String>,
    },
}

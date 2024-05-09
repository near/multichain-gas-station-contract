use ethers_core::types::U256;
use near_sdk::AccountId;
use thiserror::Error;

#[derive(Debug, Error, Clone)]
#[error("Configuration for chain ID {chain_id} does not exist")]
pub struct ChainConfigurationDoesNotExistError {
    pub chain_id: u64,
}

#[derive(Debug, Error, Clone)]
#[error("Transaction sequence with ID {transaction_sequence_id} does not exist")]
pub struct TransactionSequenceDoesNotExistError {
    pub transaction_sequence_id: u64,
}

#[derive(Debug, Error, Clone)]
#[error("Signature request {transaction_sequence_id}.{index} does not exist")]
pub struct SignatureRequestDoesNoteExistError {
    pub transaction_sequence_id: u64,
    pub index: u32,
}

#[derive(Debug, Error, Clone)]
#[error("Paymaster does not have enough funds: minimum available {minimum_available_balance} < amount {amount}")]
pub struct PaymasterInsufficientFundsError {
    pub minimum_available_balance: U256,
    pub amount: U256,
}

#[derive(Debug, Error, Clone)]
#[error("No paymaster configurations exist for chain ID {chain_id}")]
pub struct NoPaymasterConfigurationForChainError {
    pub chain_id: u64,
}

#[derive(Debug, Error, Clone)]
#[error("Attached deposit is less than fee: deposit {deposit} < fee {fee}")]
pub struct InsufficientDepositForFeeError {
    pub fee: u128,
    pub deposit: u128,
}

#[derive(Debug, Error, Clone)]
#[error("Reported price is negative")]
pub struct NegativePriceError;

#[derive(Debug, Error, Clone)]
#[error("Price confidence interval is too large")]
pub struct ConfidenceIntervalTooLargeError;

#[derive(Debug, Error, Clone)]
#[error("Price exponent is too large")]
pub struct ExponentTooLargeError;

#[derive(Debug, Error, Clone)]
pub enum PriceDataError {
    #[error(transparent)]
    NegativePrice(#[from] NegativePriceError),
    #[error(transparent)]
    ConfidenceIntervalTooLarge(#[from] ConfidenceIntervalTooLargeError),
    #[error(transparent)]
    ExponentTooLarge(#[from] ExponentTooLargeError),
}

#[derive(Debug, Error, Clone)]
pub enum RequestNonceError {
    #[error(transparent)]
    NoPaymasterConfigurationForChain(#[from] NoPaymasterConfigurationForChainError),
    #[error(transparent)]
    PaymasterInsufficientFunds(#[from] PaymasterInsufficientFundsError),
}

#[derive(Debug, Error, Clone)]
#[error("Oracle query failed")]
pub struct OracleQueryFailureError;

#[derive(Debug, Error, Clone)]
#[error("Sender is unauthorized for the requested NFT chain key")]
pub struct SenderUnauthorizedForNftChainKeyError {
    pub sender: AccountId,
    pub token_id: String,
}

#[derive(Debug, Error, Clone)]
pub enum TryCreateTransactionCallbackError {
    #[error(transparent)]
    OracleQueryFailure(#[from] OracleQueryFailureError),
    #[error(transparent)]
    SenderUnauthorizedForNftChainKey(#[from] SenderUnauthorizedForNftChainKeyError),
    #[error(transparent)]
    ChainConfigurationDoesNotExist(#[from] ChainConfigurationDoesNotExistError),
    #[error(transparent)]
    PriceData(#[from] PriceDataError),
    #[error(transparent)]
    InsufficientDepositForFee(#[from] InsufficientDepositForFeeError),
    #[error(transparent)]
    RequestNonce(#[from] RequestNonceError),
}

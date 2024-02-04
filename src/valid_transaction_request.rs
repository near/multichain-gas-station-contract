use ethers_core::types::{
    transaction::eip2718::TypedTransaction, NameOrAddress, TransactionRequest, U256, U64,
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    serde::{Deserialize, Serialize},
};
use schemars::JsonSchema;
use thiserror::Error;

use crate::foreign_address::ForeignAddress;

#[derive(
    Serialize,
    Deserialize,
    BorshSerialize,
    BorshDeserialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    JsonSchema,
)]
#[serde(crate = "near_sdk::serde")]
pub struct ValidTransactionRequest {
    pub receiver: ForeignAddress,
    pub gas: [u64; 4],
    pub gas_price: [u64; 4],
    pub value: [u64; 4],
    pub data: Vec<u8>,
    pub nonce: [u64; 4],
    pub chain_id: u64,
}

impl TryFrom<TransactionRequest> for ValidTransactionRequest {
    type Error = TransactionValidationError;

    fn try_from(transaction: TransactionRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            receiver: transaction
                .to
                .ok_or(TransactionValidationError::Missing("to"))?
                .as_address()
                .ok_or(TransactionValidationError::InvalidReceiver)?
                .into(),
            gas: transaction
                .gas
                .ok_or(TransactionValidationError::Missing("gas"))?
                .0,
            gas_price: transaction
                .gas_price
                .ok_or(TransactionValidationError::Missing("gas_price"))?
                .0,
            value: transaction
                .value
                .ok_or(TransactionValidationError::Missing("value"))?
                .0,
            data: transaction.data.map_or_else(Vec::new, |d| d.to_vec()),
            nonce: transaction
                .nonce
                .ok_or(TransactionValidationError::Missing("nonce"))?
                .0,
            chain_id: transaction
                .chain_id
                .ok_or(TransactionValidationError::Missing("chain_id"))?
                .as_u64(),
        })
    }
}

impl ValidTransactionRequest {
    pub fn gas(&self) -> U256 {
        U256(self.gas)
    }
    pub fn gas_price(&self) -> U256 {
        U256(self.gas_price)
    }
    pub fn value(&self) -> U256 {
        U256(self.value)
    }
    pub fn nonce(&self) -> U256 {
        U256(self.nonce)
    }
    pub fn chain_id(&self) -> U64 {
        U64([self.chain_id])
    }
}

impl From<ValidTransactionRequest> for TransactionRequest {
    fn from(transaction: ValidTransactionRequest) -> Self {
        Self {
            from: None,
            to: Some(NameOrAddress::Address(transaction.receiver.into())),
            gas: Some(transaction.gas()),
            gas_price: Some(transaction.gas_price()),
            value: Some(transaction.value()),
            nonce: Some(transaction.nonce()),
            chain_id: Some(transaction.chain_id()),
            data: Some(ethers_core::types::Bytes::from(transaction.data)),
        }
    }
}

impl From<ValidTransactionRequest> for TypedTransaction {
    fn from(value: ValidTransactionRequest) -> Self {
        <TransactionRequest as From<ValidTransactionRequest>>::from(value).into()
    }
}

#[derive(Debug, Error)]
pub enum TransactionValidationError {
    #[error("Missing field: `{0}`")]
    Missing(&'static str),
    #[error("Invalid receiver")]
    InvalidReceiver,
}

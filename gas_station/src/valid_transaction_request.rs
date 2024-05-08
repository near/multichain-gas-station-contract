use ethers_core::{
    types::{
        transaction::{eip2718::TypedTransaction, eip2930::AccessList},
        Eip1559TransactionRequest, NameOrAddress, U256, U64,
    },
    utils::rlp::{Decodable, Encodable, Rlp},
};
use lib::foreign_address::ForeignAddress;
use near_sdk::near;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
#[near(serializers = [borsh, json])]
pub struct ValidTransactionRequest {
    pub to: ForeignAddress,
    pub gas: [u64; 4],
    pub value: [u64; 4],
    pub data: Vec<u8>,
    pub nonce: [u64; 4],
    pub access_list_rlp: Vec<u8>,
    pub max_priority_fee_per_gas: [u64; 4],
    pub max_fee_per_gas: [u64; 4],
    pub chain_id: u64,
}

impl TryFrom<Eip1559TransactionRequest> for ValidTransactionRequest {
    type Error = TransactionValidationError;

    fn try_from(transaction: Eip1559TransactionRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            to: transaction
                .to
                .ok_or(TransactionValidationError::Missing("to"))?
                .as_address()
                .ok_or(TransactionValidationError::InvalidReceiver)?
                .into(),
            gas: transaction
                .gas
                .ok_or(TransactionValidationError::Missing("gas"))?
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
            access_list_rlp: transaction.access_list.rlp_bytes().to_vec(),
            max_priority_fee_per_gas: transaction
                .max_priority_fee_per_gas
                .ok_or(TransactionValidationError::Missing(
                    "max_priority_fee_per_gas",
                ))?
                .0,
            max_fee_per_gas: transaction
                .max_fee_per_gas
                .ok_or(TransactionValidationError::Missing("max_fee_per_gas"))?
                .0,
            chain_id: transaction
                .chain_id
                .ok_or(TransactionValidationError::Missing("chain_id"))?
                .as_u64(),
        })
    }
}

impl ValidTransactionRequest {
    #[must_use]
    pub fn gas(&self) -> U256 {
        U256(self.gas)
    }

    #[must_use]
    pub fn max_fee_per_gas(&self) -> U256 {
        U256(self.max_fee_per_gas)
    }

    #[must_use]
    pub fn max_priority_fee_per_gas(&self) -> U256 {
        U256(self.max_priority_fee_per_gas)
    }

    /// Attempt to parse the access list.
    ///
    /// # Errors
    ///
    /// Returns an error if the access list could not be decoded.
    pub fn access_list(&self) -> Result<AccessList, ethers_core::utils::rlp::DecoderError> {
        AccessList::decode(&Rlp::new(&self.access_list_rlp))
    }

    #[must_use]
    pub fn value(&self) -> U256 {
        U256(self.value)
    }

    #[must_use]
    pub fn nonce(&self) -> U256 {
        U256(self.nonce)
    }

    #[must_use]
    pub fn chain_id(&self) -> U64 {
        U64([self.chain_id])
    }

    /// Useful because transaction types annoyingly have a local function called `from`.
    #[must_use]
    pub fn into_typed_transaction(self) -> TypedTransaction {
        <Eip1559TransactionRequest as From<ValidTransactionRequest>>::from(self).into()
    }
}

impl From<ValidTransactionRequest> for Eip1559TransactionRequest {
    fn from(transaction: ValidTransactionRequest) -> Self {
        Self {
            from: None,
            access_list: transaction.access_list().unwrap(),
            max_priority_fee_per_gas: Some(transaction.max_priority_fee_per_gas()),
            max_fee_per_gas: Some(transaction.max_fee_per_gas()),
            to: Some(NameOrAddress::Address(transaction.to.into())),
            gas: Some(transaction.gas()),
            value: Some(transaction.value()),
            nonce: Some(transaction.nonce()),
            chain_id: Some(transaction.chain_id()),
            data: Some(ethers_core::types::Bytes::from(transaction.data)),
        }
    }
}

impl From<ValidTransactionRequest> for TypedTransaction {
    fn from(value: ValidTransactionRequest) -> Self {
        <Eip1559TransactionRequest as From<ValidTransactionRequest>>::from(value).into()
    }
}

#[derive(Debug, Error)]
pub enum TransactionValidationError {
    #[error("Missing field: `{0}`")]
    Missing(&'static str),
    #[error("Invalid receiver")]
    InvalidReceiver,
}

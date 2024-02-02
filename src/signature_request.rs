use ethers_core::types::U256;
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    serde::{Deserialize, Serialize},
};
use schemars::JsonSchema;

use crate::valid_transaction_request::ValidTransactionRequest;

#[derive(
    Serialize,
    Deserialize,
    BorshSerialize,
    BorshDeserialize,
    JsonSchema,
    Default,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(crate = "near_sdk::serde")]
pub struct SignatureBorsh {
    r: [u8; 32],
    s: [u8; 32],
    v: u8,
}

impl From<ethers_core::types::Signature> for SignatureBorsh {
    fn from(signature: ethers_core::types::Signature) -> Self {
        let mut r = [0u8; 32];
        signature.r.to_big_endian(&mut r);
        let mut s = [0u8; 32];
        signature.s.to_big_endian(&mut s);
        let v = signature.v as u8;
        Self { r, s, v }
    }
}

impl From<SignatureBorsh> for ethers_core::types::Signature {
    fn from(signature: SignatureBorsh) -> Self {
        ethers_core::types::Signature {
            r: U256::from_big_endian(&signature.r),
            s: U256::from_big_endian(&signature.s),
            v: signature.v as u64,
        }
    }
}

#[derive(
    BorshDeserialize,
    BorshSerialize,
    Serialize,
    Deserialize,
    JsonSchema,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(crate = "near_sdk::serde")]
pub enum SignatureRequestStatus {
    Pending { key_path: String, in_flight: bool },
    Signed { signature: SignatureBorsh },
}

#[derive(
    BorshDeserialize,
    BorshSerialize,
    Serialize,
    Deserialize,
    JsonSchema,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(crate = "near_sdk::serde")]
pub struct SignatureRequest {
    pub status: SignatureRequestStatus,
    pub transaction: ValidTransactionRequest,
}

impl SignatureRequest {
    pub fn new(key_path: &impl ToString, transaction: ValidTransactionRequest) -> Self {
        Self {
            status: SignatureRequestStatus::Pending {
                key_path: key_path.to_string(),
                in_flight: false,
            },
            transaction,
        }
    }

    pub const fn is_pending(&self) -> bool {
        matches!(self.status, SignatureRequestStatus::Pending { .. })
    }

    pub const fn is_signed(&self) -> bool {
        matches!(self.status, SignatureRequestStatus::Signed { .. })
    }

    pub fn set_signature(&mut self, signature: impl Into<SignatureBorsh>) {
        self.status = SignatureRequestStatus::Signed {
            signature: signature.into(),
        };
    }
}

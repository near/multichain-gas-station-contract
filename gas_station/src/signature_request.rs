use ethers_core::types::U256;
use near_sdk::near;

use crate::{valid_transaction_request::ValidTransactionRequest, ChainKeyAuthorization};

#[derive(Default, Debug, Clone, PartialEq, Eq)]
#[near(serializers = [borsh, json])]
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

        // permissible due to the runtime guarantees provided by the `Signature` type
        #[allow(clippy::cast_possible_truncation)]
        let v = signature.v as u8;
        Self { r, s, v }
    }
}

impl From<SignatureBorsh> for ethers_core::types::Signature {
    fn from(signature: SignatureBorsh) -> Self {
        ethers_core::types::Signature {
            r: U256::from_big_endian(&signature.r),
            s: U256::from_big_endian(&signature.s),
            v: u64::from(signature.v),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[near(serializers = [borsh, json])]
pub enum Status {
    Pending,
    InFlight,
    Signed { signature: SignatureBorsh },
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[near(serializers = [borsh, json])]
pub struct SignatureRequest {
    pub status: Status,
    pub token_id: String,
    pub authorization: ChainKeyAuthorization,
    pub is_paymaster: bool,
    pub transaction: ValidTransactionRequest,
}

impl SignatureRequest {
    pub fn new(
        token_id: &impl ToString,
        authorization: ChainKeyAuthorization,
        transaction: ValidTransactionRequest,
        is_paymaster: bool,
    ) -> Self {
        Self {
            status: Status::Pending,
            token_id: token_id.to_string(),
            authorization,
            is_paymaster,
            transaction,
        }
    }

    #[must_use]
    pub const fn is_pending(&self) -> bool {
        matches!(self.status, Status::Pending { .. })
    }

    #[must_use]
    pub const fn is_in_flight(&self) -> bool {
        matches!(self.status, Status::InFlight { .. })
    }

    #[must_use]
    pub const fn is_signed(&self) -> bool {
        matches!(self.status, Status::Signed { .. })
    }

    pub fn set_signature(&mut self, signature: impl Into<SignatureBorsh>) {
        self.status = Status::Signed {
            signature: signature.into(),
        };
    }
}

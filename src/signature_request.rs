use ethers::{
    types::transaction::eip2718::TypedTransaction,
    utils::rlp::{Decodable, Rlp},
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    serde::{Deserialize, Serialize},
};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(
    crate = "near_sdk::serde",
    from = "TypedTransaction",
    into = "TypedTransaction"
)]
pub struct TypedTransactionBorsh(pub TypedTransaction);
// borsh_via_rlp!(TypedTransactionBorsh, TypedTransaction);

impl From<TypedTransaction> for TypedTransactionBorsh {
    fn from(transaction: TypedTransaction) -> Self {
        TypedTransactionBorsh(transaction)
    }
}

impl From<TypedTransactionBorsh> for TypedTransaction {
    fn from(transaction: TypedTransactionBorsh) -> Self {
        transaction.0
    }
}

impl BorshSerialize for TypedTransactionBorsh {
    fn serialize<W: std::io::prelude::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        BorshSerialize::serialize(&self.0.rlp().to_vec(), writer)
    }
}

impl BorshDeserialize for TypedTransactionBorsh {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        let bytes = <Vec<u8> as BorshDeserialize>::deserialize(buf)?;
        let rlp = Rlp::new(&bytes);
        let transaction = TypedTransaction::decode(&rlp).unwrap();
        Ok(TypedTransactionBorsh(transaction))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(
    crate = "near_sdk::serde",
    from = "ethers::types::Signature",
    into = "ethers::types::Signature"
)]
pub struct SignatureBorsh(pub ethers::types::Signature);

impl From<ethers::types::Signature> for SignatureBorsh {
    fn from(signature: ethers::types::Signature) -> Self {
        Self(signature)
    }
}

impl From<SignatureBorsh> for ethers::types::Signature {
    fn from(signature: SignatureBorsh) -> Self {
        signature.0
    }
}

impl BorshSerialize for SignatureBorsh {
    fn serialize<W: std::io::prelude::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        BorshSerialize::serialize(&<[u8; 65]>::from(self.0), writer)
    }
}

impl BorshDeserialize for SignatureBorsh {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        let bytes = <[u8; 65] as BorshDeserialize>::deserialize(buf)?;
        Ok(Self(
            ethers::types::Signature::try_from(&bytes[..]).unwrap(),
        ))
    }
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub enum SignatureRequestStatus {
    Pending { key_path: String, in_flight: bool },
    Signed { signature: SignatureBorsh },
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct SignatureRequest {
    pub status: SignatureRequestStatus,
    pub transaction: TypedTransactionBorsh,
}

impl SignatureRequest {
    pub fn new(key_path: impl ToString, transaction: impl Into<TypedTransactionBorsh>) -> Self {
        Self {
            status: SignatureRequestStatus::Pending {
                key_path: key_path.to_string(),
                in_flight: false,
            },
            transaction: transaction.into(),
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

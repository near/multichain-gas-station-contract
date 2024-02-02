use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    ext_contract,
    serde::{Deserialize, Serialize},
    PromiseOrValue, PublicKey,
};
use schemars::JsonSchema;
use thiserror::Error;

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct InitializingContractState {}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct RunningContractState {
    pub public_key: PublicKey,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct ResharingContractState {
    pub public_key: PublicKey,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug)]
#[serde(crate = "near_sdk::serde")]
pub enum ProtocolContractState {
    NotInitialized,
    Initializing(InitializingContractState),
    Running(RunningContractState),
    Resharing(ResharingContractState),
}

#[allow(clippy::ptr_arg)]
#[ext_contract(ext_signer)]
pub trait SignerContract {
    fn sign(&mut self, payload: [u8; 32], path: &String) -> PromiseOrValue<MpcSignature>;
    fn state(&self) -> ProtocolContractState;
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
#[serde(crate = "near_sdk::serde")]
pub struct MpcSignature(String, String);

impl MpcSignature {
    pub fn new(r: [u8; 32], s: [u8; 32], v: u8) -> Self {
        let mut big_r = [0u8; 33];
        big_r[0] = v;
        big_r[1..].copy_from_slice(&r);

        Self(hex::encode(big_r), hex::encode(s))
    }
}

#[derive(Debug, Error)]
pub enum MpcSignatureDecodeError {
    #[error("Hex decoding error")]
    Hex(#[from] hex::FromHexError),
    #[error("Invalid length")]
    InvalidLength,
}

impl TryFrom<MpcSignature> for ethers_core::types::Signature {
    type Error = MpcSignatureDecodeError;

    fn try_from(MpcSignature(big_r_hex, s_hex): MpcSignature) -> Result<Self, Self::Error> {
        let s_bytes = hex::decode(s_hex)?;
        let s: &[u8; 32] = s_bytes
            .as_slice()
            .try_into()
            .map_err(|_| MpcSignatureDecodeError::InvalidLength)?;

        let big_r_bytes = hex::decode(big_r_hex)?;
        let (v, r) = match &big_r_bytes[..] {
            [v, r @ ..] => (*v, r),
            _ => return Err(MpcSignatureDecodeError::InvalidLength),
        };

        let r: &[u8; 32] = r
            .try_into()
            .map_err(|_| MpcSignatureDecodeError::InvalidLength)?;

        Ok(ethers_core::types::Signature {
            r: r.into(),
            s: s.into(),
            v: v.into(),
        })
    }
}

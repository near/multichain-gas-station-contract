use ethers_core::k256::{
    self,
    ecdsa::RecoveryId,
    elliptic_curve::{
        self, group::GroupEncoding, ops::Reduce, point::AffineCoordinates, PrimeField,
    },
    AffinePoint, Secp256k1,
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    ext_contract,
    serde::{Deserialize, Serialize},
    PromiseOrValue, PublicKey,
};
use schemars::JsonSchema;
use thiserror::Error;

use crate::kdf::ScalarExt;

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
    fn public_key(&self) -> near_sdk::PublicKey;
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
#[serde(crate = "near_sdk::serde")]
pub struct MpcSignature(pub String, pub String);

impl MpcSignature {
    #[must_use]
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
    #[error("Invalid signature data")]
    InvalidSignatureData,
}

impl TryFrom<MpcSignature> for ethers_core::types::Signature {
    type Error = MpcSignatureDecodeError;

    fn try_from(MpcSignature(big_r_hex, s_hex): MpcSignature) -> Result<Self, Self::Error> {
        let big_r = Option::<AffinePoint>::from(AffinePoint::from_bytes(
            hex::decode(big_r_hex)?[..].into(),
        ))
        .ok_or(MpcSignatureDecodeError::InvalidSignatureData)?;
        let s = k256::Scalar::from_bytes(&hex::decode(s_hex)?);

        let r = <k256::Scalar as Reduce<<Secp256k1 as elliptic_curve::Curve>::Uint>>::reduce_bytes(
            &big_r.x(),
        );
        let x_is_reduced = r.to_repr() != big_r.x();

        let v = RecoveryId::new(big_r.y_is_odd().into(), x_is_reduced);

        Ok(ethers_core::types::Signature {
            r: r.to_bytes().as_slice().into(),
            s: s.to_bytes().as_slice().into(),
            v: v.to_byte().into(),
        })
    }
}

use ethers_core::{
    k256::{
        self,
        ecdsa::RecoveryId,
        elliptic_curve::{
            self,
            group::GroupEncoding,
            ops::Reduce,
            point::{AffineCoordinates, DecompressPoint},
            PrimeField,
        },
        AffinePoint, Secp256k1,
    },
    utils::hex,
};
use near_sdk::{
    ext_contract,
    serde::{Deserialize, Serialize},
    PromiseOrValue,
};
use schemars::JsonSchema;
use thiserror::Error;

#[allow(clippy::ptr_arg)]
#[ext_contract(ext_signer)]
pub trait SignerInterface {
    fn sign(
        &mut self,
        payload: [u8; 32],
        path: &String,
        key_version: u16,
    ) -> PromiseOrValue<MpcSignature>;
    fn public_key(&self) -> near_sdk::PublicKey;
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
#[serde(crate = "near_sdk::serde")]
pub struct MpcSignature(pub String, pub String);

impl MpcSignature {
    #[must_use]
    pub fn new(r: [u8; 32], s: [u8; 32], v: RecoveryId) -> Option<Self> {
        let big_r = Option::<AffinePoint>::from(AffinePoint::decompress(
            &r.into(),
            u8::from(v.is_y_odd()).into(),
        ))?;

        Some(Self(hex::encode(big_r.to_bytes()), hex::encode(s)))
    }

    #[must_use]
    pub fn from_ecdsa_signature(
        signature: ethers_core::k256::ecdsa::Signature,
        recovery_id: RecoveryId,
    ) -> Option<Self> {
        MpcSignature::new(
            signature.r().to_bytes().into(),
            signature.s().to_bytes().into(),
            recovery_id,
        )
    }
}

#[derive(Debug, Error)]
pub enum MpcSignatureDecodeError {
    #[error("Failed to decode signature from hex: {0}")]
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
        let s = hex::decode(s_hex)?;

        let r = <k256::Scalar as Reduce<<Secp256k1 as elliptic_curve::Curve>::Uint>>::reduce_bytes(
            &big_r.x(),
        );
        let x_is_reduced = r.to_repr() != big_r.x();

        let v = RecoveryId::new(big_r.y_is_odd().into(), x_is_reduced);

        Ok(ethers_core::types::Signature {
            r: r.to_bytes().as_slice().into(),
            s: s.as_slice().into(),
            v: v.to_byte().into(),
        })
    }
}

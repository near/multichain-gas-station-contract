// From: https://github.com/near/mpc-recovery/blob/bc85d66833ffa8537ec61d0b22cd5aa96fbe3197/node/src/kdf.rs

use ethers_core::k256::elliptic_curve::sec1::ToEncodedPoint;
// use crate::types::PublicKey;
// use crate::util::ScalarExt;
use ethers_core::k256::elliptic_curve::{scalar::*, CurveArithmetic};
use ethers_core::k256::{Scalar, Secp256k1, U256};
use near_sdk::AccountId;

pub type PublicKey = <Secp256k1 as CurveArithmetic>::AffinePoint;

pub trait ScalarExt {
    fn from_bytes(bytes: &[u8]) -> Self;
}

impl ScalarExt for Scalar {
    fn from_bytes(bytes: &[u8]) -> Self {
        Scalar::from_uint_unchecked(U256::from_le_slice(bytes))
    }
}

#[cfg(target_family = "wasm")]
fn sha256(bytes: &[u8]) -> Vec<u8> {
    near_sdk::env::sha256(bytes)
}

#[cfg(not(target_family = "wasm"))]
fn sha256(bytes: &[u8]) -> Vec<u8> {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    hasher.finalize().to_vec()
}

// Constant prefix that ensures epsilon derivation values are used specifically for
// near-mpc-recovery with key derivation protocol vX.Y.Z.
const EPSILON_DERIVATION_PREFIX: &str = "near-mpc-recovery v0.1.0 epsilon derivation:";

pub fn derive_epsilon(signer_id: &AccountId, path: &str) -> Scalar {
    let derivation_path = format!("{EPSILON_DERIVATION_PREFIX}{},{}", signer_id, path);
    Scalar::from_bytes(&sha256(derivation_path.as_bytes()))
}

pub fn derive_key(public_key: PublicKey, epsilon: Scalar) -> PublicKey {
    (<Secp256k1 as CurveArithmetic>::ProjectivePoint::GENERATOR * epsilon + public_key).to_affine()
}

pub fn get_mpc_address(
    mpc_public_key: PublicKey,
    account_id: &AccountId,
    path: &str,
) -> ethers_core::types::Address {
    let epsilon = derive_epsilon(account_id, path);
    let affine_point = derive_key(mpc_public_key, epsilon);
    let encoded = affine_point.to_encoded_point(false);
    ethers_core::utils::raw_public_key_to_address(&encoded.as_bytes()[1..])
}

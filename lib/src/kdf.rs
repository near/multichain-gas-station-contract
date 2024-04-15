// From: https://github.com/near/mpc-recovery/blob/bc85d66833ffa8537ec61d0b22cd5aa96fbe3197/node/src/kdf.rs

use ethers_core::k256::{
    elliptic_curve::{
        scalar::FromUintUnchecked,
        sec1::{FromEncodedPoint, Tag, ToEncodedPoint},
        CurveArithmetic,
    },
    AffinePoint, EncodedPoint, Scalar, Secp256k1, U256,
};
use near_sdk::AccountId;

use crate::foreign_address::ForeignAddress;

pub type PublicKey = <Secp256k1 as CurveArithmetic>::AffinePoint;

#[cfg(target_arch = "wasm32")]
pub fn sha256(bytes: &[u8]) -> Vec<u8> {
    near_sdk::env::sha256(bytes)
}

#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn sha256(bytes: &[u8]) -> Vec<u8> {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    hasher.finalize().to_vec()
}

// Constant prefix that ensures epsilon derivation values are used specifically for
// near-mpc-recovery with key derivation protocol vX.Y.Z.
const EPSILON_DERIVATION_PREFIX: &str = "near-mpc-recovery v0.1.0 epsilon derivation:";

#[must_use]
pub fn derive_epsilon(signer_id: &AccountId, path: &str) -> Scalar {
    let derivation_path = format!("{EPSILON_DERIVATION_PREFIX}{signer_id},{path}");
    Scalar::from_uint_unchecked(U256::from_le_slice(&sha256(derivation_path.as_bytes())))
}

#[must_use]
pub fn derive_key(public_key: PublicKey, epsilon: Scalar) -> PublicKey {
    (<Secp256k1 as CurveArithmetic>::ProjectivePoint::GENERATOR * epsilon + public_key).to_affine()
}

#[must_use]
pub fn derive_evm_address_for_account(
    mpc_public_key: PublicKey,
    account_id: &AccountId,
    path: &str,
) -> ethers_core::types::Address {
    let epsilon = derive_epsilon(account_id, path);
    let affine_point = derive_key(mpc_public_key, epsilon);
    let encoded = affine_point.to_encoded_point(false);
    let encoded_bytes = encoded.as_bytes();
    ethers_core::utils::raw_public_key_to_address(&encoded_bytes[1..])
}

#[derive(Debug, thiserror::Error)]
pub enum PublicKeyConversionError {
    #[error("Can only convert from SECP256K1")]
    WrongCurveType(near_sdk::CurveType),
    #[error("Decoding error")]
    DecodingError(#[from] ethers_core::k256::elliptic_curve::Error),
    #[error("Invalid key data")]
    InvalidKeyData,
}

/// Converts an SECP256K1-variant [`near_sdk::PublicKey`] to an [`AffinePoint`].
///
/// # Errors
///
/// Returns an error if the public key is not a valid SECP256K1 key.
pub fn near_public_key_to_affine(
    public_key: near_sdk::PublicKey,
) -> Result<AffinePoint, PublicKeyConversionError> {
    // wasm only
    #[cfg(target_arch = "wasm32")]
    {
        let curve_type = public_key.curve_type();
        if curve_type != near_sdk::CurveType::SECP256K1 {
            return Err(PublicKeyConversionError::WrongCurveType(curve_type));
        }
    }

    let mut bytes = public_key.into_bytes();
    bytes[0] = u8::from(Tag::Uncompressed);

    let affine: Option<AffinePoint> = AffinePoint::from_encoded_point(
        &EncodedPoint::from_bytes(&bytes)
            .map_err(|e| PublicKeyConversionError::DecodingError(e.into()))?,
    )
    .into();

    affine.ok_or(PublicKeyConversionError::InvalidKeyData)
}

/// Calculates the public key of the MPC signer for the given account ID and derivation path.
///
/// # Errors
///
/// Returns an error if the public key is not a valid SECP256K1 key.
pub fn get_mpc_address(
    mpc_public_key: near_sdk::PublicKey,
    gas_station_account_id: &AccountId,
    caller_account_id: &str,
) -> Result<ForeignAddress, PublicKeyConversionError> {
    let affine = near_public_key_to_affine(mpc_public_key)?;
    Ok(derive_evm_address_for_account(affine, gas_station_account_id, caller_account_id).into())
}

/// Calculates the encoded point for a given MPC public key, predecessor, and key path.
///
/// # Errors
///
/// Returns an error if the public key is not a valid SECP256K1 key.
pub fn derive_public_key_for(
    mpc_public_key: near_sdk::PublicKey,
    predecessor_account_id: &AccountId,
    path: &str,
) -> Result<EncodedPoint, PublicKeyConversionError> {
    let affine = near_public_key_to_affine(mpc_public_key)?;
    let epsilon = derive_epsilon(predecessor_account_id, path);
    let affine_point = derive_key(affine, epsilon);

    Ok(affine_point.to_encoded_point(false))
}

#[test]
fn test_keys() {
    let public_key: near_sdk::PublicKey = "secp256k1:47xve2ymatpG4x4Gp7pmYwuLJk7eeRegrFuS4VoW5VV4i3GsBiBY87vkH6UZiiY18NeZnkBzcZzipDbJJ5pmjTcc"
        .parse()
        .unwrap();

    let a = near_public_key_to_affine(public_key.clone()).unwrap();

    let encoded = a.to_encoded_point(false);
    println!("{encoded:x}");

    let mpc_address = derive_evm_address_for_account(a, &"canhazgas.testnet".parse().unwrap(), "");

    println!("{}", ethers_core::utils::to_checksum(&mpc_address, None));
}

// The below tests confirm parity with https://gist.github.com/esaminu/f8cc37849de754f228c5a67bebce9b0f

#[test]
fn test_derive_epsilon() {
    let epsilon = derive_epsilon(&"canhazgas.testnet".parse().unwrap(), "");
    let b = epsilon.to_bytes();
    assert_eq!(
        ethers_core::utils::hex::encode_prefixed(b.as_slice()),
        "0x2f11aa32079bf3f96684143a68e66c47b83afd6fc721999989543ad1a16f948d"
    );
}

#[test]
fn test_derive_key() {
    let parent_public_key_bytes = ethers_core::utils::hex::decode("0x049c0e823c86c14a5810d00c2d584c0b787337bff65a55465febfc15dbaba509f1e46ec19c2b85e8fb6df520df8234127617c94d302abeaed2d2ae1170562e87e9").unwrap();
    let parent_encoded_point = EncodedPoint::from_bytes(parent_public_key_bytes).unwrap();
    let parent_affine_point = AffinePoint::from_encoded_point(&parent_encoded_point).unwrap();
    let epsilon = derive_epsilon(&"canhazgas.testnet".parse().unwrap(), "");
    let derived_key = derive_key(parent_affine_point, epsilon);
    let derived_key_encoded_point = derived_key.to_encoded_point(false);
    assert_eq!(
        ethers_core::utils::hex::encode_prefixed(derived_key_encoded_point.as_bytes()),
        "0x04762ab28d3efef07ea4df3e61bafb14b9389f67a91fe3db3214132ebceef7a115644a8b87e01cb0c0cb34d78b176c7358f93a73dd7d5d885bbd598dde06e69647"
    );
}

#[test]
fn test_derive_evm_address() {
    let public_key_bytes = ethers_core::utils::hex::decode("04762ab28d3efef07ea4df3e61bafb14b9389f67a91fe3db3214132ebceef7a115644a8b87e01cb0c0cb34d78b176c7358f93a73dd7d5d885bbd598dde06e69647").unwrap();
    let evm_address = format!(
        "{:#x}",
        &ethers_core::utils::raw_public_key_to_address(&public_key_bytes[1..]),
    );
    assert_eq!(evm_address, "0x4a435791735b6295637dbf2a44bd1f9f1a5e3cbc");
}

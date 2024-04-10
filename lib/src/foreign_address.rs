use std::{fmt::Display, str::FromStr};

use ethers_core::{
    types::{NameOrAddress, H160},
    utils::to_checksum,
};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use schemars::JsonSchema;

#[derive(
    BorshSerialize, BorshDeserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone, Debug,
)]
pub struct ForeignAddress(pub [u8; 20]);

impl ForeignAddress {
    /// Creates a new [`ForeignAddress`] from the provided public key bytes.
    ///
    /// # Panics
    ///
    /// Panics if provided `key` is not a valid public key.
    pub fn from_raw_public_key(key_bytes: impl AsRef<[u8]>) -> Self {
        ethers_core::utils::raw_public_key_to_address(&key_bytes.as_ref()[1..]).into()
    }
}

impl near_sdk::serde::Serialize for ForeignAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: near_sdk::serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> near_sdk::serde::Deserialize<'de> for ForeignAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: near_sdk::serde::Deserializer<'de>,
    {
        let s = <String as near_sdk::serde::Deserialize>::deserialize(deserializer)?;
        ForeignAddress::from_str(&s).map_err(near_sdk::serde::de::Error::custom)
    }
}

impl JsonSchema for ForeignAddress {
    fn schema_name() -> String {
        String::schema_name()
    }

    fn json_schema(gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        String::json_schema(gen)
    }

    fn is_referenceable() -> bool {
        false
    }
}

impl Display for ForeignAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", to_checksum(&self.0.into(), None))
    }
}

impl AsRef<[u8]> for ForeignAddress {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<&H160> for ForeignAddress {
    fn from(value: &H160) -> Self {
        Self(value.0)
    }
}

impl From<H160> for ForeignAddress {
    fn from(value: H160) -> Self {
        Self(value.0)
    }
}

impl From<ForeignAddress> for H160 {
    fn from(value: ForeignAddress) -> Self {
        Self(value.0)
    }
}

impl From<ForeignAddress> for NameOrAddress {
    fn from(value: ForeignAddress) -> Self {
        Self::Address(value.into())
    }
}

impl FromStr for ForeignAddress {
    type Err = ethers_core::utils::ConversionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(ethers_core::utils::parse_checksummed(s, None)?.0))
    }
}

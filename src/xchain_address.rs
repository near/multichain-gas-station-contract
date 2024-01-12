use std::{fmt::Display, str::FromStr};

use ethers::{types::H160, utils::to_checksum};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Debug)]
pub struct XChainAddress(pub [u8; 20]);

impl near_sdk::serde::Serialize for XChainAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: near_sdk::serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> near_sdk::serde::Deserialize<'de> for XChainAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: near_sdk::serde::Deserializer<'de>,
    {
        let s = <String as near_sdk::serde::Deserialize>::deserialize(deserializer)?;
        XChainAddress::from_str(&s).map_err(near_sdk::serde::de::Error::custom)
    }
}

impl Display for XChainAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", to_checksum(&self.0.into(), None))
    }
}

impl AsRef<[u8]> for XChainAddress {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<&H160> for XChainAddress {
    fn from(value: &H160) -> Self {
        Self(value.0)
    }
}

impl From<H160> for XChainAddress {
    fn from(value: H160) -> Self {
        Self(value.0)
    }
}

impl From<XChainAddress> for H160 {
    fn from(value: XChainAddress) -> Self {
        Self(value.0)
    }
}

impl FromStr for XChainAddress {
    type Err = ethers::utils::ConversionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(ethers::utils::parse_checksummed(s, None)?.0))
    }
}

use ethers::types::H160;
use near_sdk::borsh::{BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Debug)]
#[borsh(crate = "near_sdk::borsh")]
pub struct XChainAddress([u8; 20]);

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

use ethers::types::H160;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Debug)]
pub struct XChainAddress(pub [u8; 20]);

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

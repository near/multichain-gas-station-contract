use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    serde::{Deserialize, Serialize},
};

#[derive(Serialize, Deserialize, BorshSerialize, BorshDeserialize, Clone, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct NftKeyExtraMetadata {
    pub public_key: String,
}

#[derive(Serialize, Deserialize, BorshSerialize, BorshDeserialize, Clone, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct NftKeyMinted {
    pub key_path: String,
    pub public_key: String,
}

use near_sdk::env;

use crate::xchain_address::XChainAddress;
use crate::XChainTokenAmount;

use ethers::utils::to_checksum;
use ethers::types::transaction::eip2718::TypedTransaction;

pub(crate) fn sender_address(tx: &TypedTransaction) -> Option<String> {
    tx.from().map(|from| to_checksum(from, None))
}

pub(crate) fn tokens_for_gas(tx: &TypedTransaction) -> Option<XChainTokenAmount> {
    tx.gas()
        .zip(tx.gas_price())
        .map(|(gas, gas_price)| gas * gas_price)
}

/// Rejects transaction on decoding error
pub(crate) fn address_from_hex(address: impl AsRef<[u8]>) -> XChainAddress {
    XChainAddress(
        hex::decode(address)
            .unwrap_or_else(|_| env::panic_str("Error decoding address as hex"))
            .try_into()
            .unwrap_or_else(|_| env::panic_str("Address must be 20 bytes")),
    )
}

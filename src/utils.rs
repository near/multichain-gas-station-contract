use ethers::types::{transaction::eip2718::TypedTransaction, U256};

use crate::ForeignChainTokenAmount;

pub(crate) fn foreign_tokens_for_gas(
    tx: &TypedTransaction,
    extra_gas: U256,
) -> Option<ForeignChainTokenAmount> {
    tx.gas()
        .zip(tx.gas_price())
        .map(|(gas, gas_price)| (gas + extra_gas) * gas_price)
}

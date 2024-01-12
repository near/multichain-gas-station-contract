use crate::XChainTokenAmount;

use ethers::types::transaction::eip2718::TypedTransaction;

pub(crate) fn tokens_for_gas(tx: &TypedTransaction) -> Option<XChainTokenAmount> {
    tx.gas()
        .zip(tx.gas_price())
        .map(|(gas, gas_price)| gas * gas_price)
}

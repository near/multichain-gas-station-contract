use ethers_core::{
    types::{transaction::eip2718::TypedTransaction, Eip1559TransactionRequest},
    utils::{
        hex,
        rlp::{Decodable, Rlp},
    },
};
use lib::Rejectable;

use crate::valid_transaction_request::ValidTransactionRequest;

pub fn decode_transaction_request(rlp_hex: &str) -> Eip1559TransactionRequest {
    let rlp_bytes =
        hex::decode(rlp_hex).expect_or_reject("Error decoding `transaction_rlp` as hex");
    let rlp = Rlp::new(&rlp_bytes);
    Eip1559TransactionRequest::decode(&rlp)
        .expect_or_reject("Error decoding `transaction_rlp` as transaction request RLP")
}

pub fn sighash_for_mpc_signing(signature_request: ValidTransactionRequest) -> [u8; 32] {
    let mut sighash =
        <TypedTransaction as From<ValidTransactionRequest>>::from(signature_request.clone())
            .sighash()
            .to_fixed_bytes();
    sighash.reverse();
    sighash
}

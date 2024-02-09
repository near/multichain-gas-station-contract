// NOTE: If tests fail due to a directory not existing error, create `target/near/{oracle,signer}`

use ethers_core::{
    k256::{
        ecdsa::RecoveryId,
        elliptic_curve::{
            bigint::Uint, group::GroupEncoding, ops::Reduce, point::AffineCoordinates, PrimeField,
        },
    },
    types::transaction::eip2718::TypedTransaction,
    utils::rlp::Rlp,
};
use near_multichain_gas_station_contract::{
    chain_configuration::PaymasterConfiguration, foreign_address::ForeignAddress, kdf::ScalarExt,
    signer_contract::MpcSignature, valid_transaction_request::ValidTransactionRequest,
    TransactionCreation,
};
use near_sdk::serde_json::json;
use near_workspaces::{
    operations::Function,
    types::{Gas, NearToken},
};

#[tokio::test]
async fn test() {
    let w = near_workspaces::sandbox().await.unwrap();

    let (gas_station, oracle, signer) = tokio::join!(
        async {
            let wasm = near_workspaces::compile_project("./").await.unwrap();
            w.dev_deploy(&wasm).await.unwrap()
        },
        async {
            let wasm = near_workspaces::compile_project("./mock/oracle")
                .await
                .unwrap();
            w.dev_deploy(&wasm).await.unwrap()
        },
        async {
            let wasm = near_workspaces::compile_project("./mock/signer")
                .await
                .unwrap();
            w.dev_deploy(&wasm).await.unwrap()
        },
    );

    println!("{:<16} {}", "Oracle:", oracle.id());
    println!("{:<16} {}", "Signer:", signer.id());
    println!("{:<16} {}", "Gas Station:", gas_station.id());

    println!("Initializing the contract...");

    gas_station
        .batch()
        .call(Function::new("new").args_json(json!({
            "signer_contract_id": signer.id(),
            "oracle_id": oracle.id(),
            "oracle_local_asset_id": "wrap.testnet",
        })))
        .call(
            Function::new("refresh_signer_public_key")
                .args_json(json!({}))
                .gas(Gas::from_tgas(50)),
        )
        .call(Function::new("add_foreign_chain").args_json(json!({
            "chain_id": "0",
            "oracle_asset_id": "weth.fakes.testnet",
            "transfer_gas": "21000",
            "fee_rate": ["120", "100"],
        })))
        .call(Function::new("add_paymaster").args_json(json!({
            "chain_id": "0",
            "nonce": 0,
            "key_path": "$",
        })))
        .transact()
        .await
        .unwrap()
        .unwrap();

    println!("Initialization complete.");

    let result = gas_station
        .view("get_paymasters")
        .args_json(json!({
            "chain_id": "0",
        }))
        .await
        .unwrap()
        .json::<Vec<PaymasterConfiguration>>()
        .unwrap();

    assert_eq!(
        result,
        vec![PaymasterConfiguration {
            nonce: 0,
            key_path: "$".to_string()
        }]
    );

    let alice = w.dev_create_account().await.unwrap();

    let eth_transaction = ethers_core::types::TransactionRequest {
        chain_id: Some(0.into()),
        from: None,
        to: Some(ForeignAddress([1; 20]).into()),
        data: None,
        gas: Some(21000.into()),
        gas_price: Some(120.into()),
        value: Some(100.into()),
        nonce: Some(0.into()),
    };

    println!("Creating transaction...");

    let tx = alice
        .call(gas_station.id(), "create_transaction")
        .args_json(json!({
            "transaction_rlp_hex": hex::encode(&eth_transaction.rlp()),
            "use_paymaster": true,
        }))
        .deposit(NearToken::from_near(1))
        .gas(Gas::from_tgas(50))
        .transact()
        .await
        .unwrap()
        .json::<TransactionCreation>()
        .unwrap();

    println!("Transaction created.");

    println!("Transaction: {tx:?}");

    assert_eq!(tx.pending_signature_count, 2, "Two signatures are pending");

    println!("Dispatching first signature...");

    let signed_tx_1 = alice
        .call(gas_station.id(), "sign_next")
        .args_json(json!({
            "id": tx.id,
        }))
        .gas(Gas::from_tgas(50))
        .transact()
        .await
        .unwrap()
        .json::<String>()
        .unwrap();

    println!("First signed transaction: {signed_tx_1:?}");

    println!("Dispatching second signature...");

    let signed_tx_2 = alice
        .call(gas_station.id(), "sign_next")
        .args_json(json!({
            "id": tx.id,
        }))
        .gas(Gas::from_tgas(50))
        .transact()
        .await
        .unwrap()
        .json::<String>()
        .unwrap();

    println!("Second signed transaction: {signed_tx_2:?}");
}

#[test]
#[ignore = "generate a payload signable by the contract"]
fn generate_eth_rlp_hex() {
    let eth_transaction = ethers_core::types::TransactionRequest {
        chain_id: Some(0.into()),
        from: None,
        to: Some(ForeignAddress([0xaa; 20]).into()),
        data: None,
        gas: Some(21000.into()),
        gas_price: Some(2.into()),
        value: Some(1111.into()),
        nonce: Some(3.into()),
    };

    println!("{}", hex::encode(eth_transaction.rlp()));
}

#[test]
fn decode_rlp() {
    let bytes = hex::decode(
        "f85f8078825208940505050505050505050505050505050505050505648003a033d5ef8c991ec82b9a6b38b7f7ca91ba34ec814c9eec1e2a42e4fc4fc9c443f7a0675e56a82d9464d1cba7ef62d7b9d6e1a4b87328c610b28dfc4b81815f8969d0",
    )
    .unwrap();

    println!("{bytes:?}");

    let rlp = Rlp::new(&bytes);

    let txrq = TypedTransaction::decode_signed(&rlp).unwrap();

    println!("{txrq:?}");
}

#[test]
fn parse_signature() {
    let t: ValidTransactionRequest = near_sdk::serde_json::from_value(json!({
        "receiver": "0x0505050505050505050505050505050505050505",
        "gas": [21000, 0, 0, 0],
        "gas_price": [120, 0, 0, 0],
        "value": [100, 0, 0, 0],
        "data": [],
        "nonce": [0, 0, 0, 0],
        "chain_id": 0
    }))
    .unwrap();
    let t = t.into_typed_transaction();

    let big_r_hex = "0333D5EF8C991EC82B9A6B38B7F7CA91BA34EC814C9EEC1E2A42E4FC4FC9C443F7";
    let s_hex = "675E56A82D9464D1CBA7EF62D7B9D6E1A4B87328C610B28DFC4B81815F8969D0";

    let big_r =
        ethers_core::k256::AffinePoint::from_bytes(hex::decode(big_r_hex).unwrap()[..].into())
            .unwrap();
    let s = ethers_core::k256::Scalar::from_bytes(&hex::decode(s_hex).unwrap());

    // let sighash = t.sighash();

    // let payload = [
    //     176u8, 195, 16, 129, 80, 137, 0, 103, 216, 40, 196, 132, 138, 70, 118, 139, 64, 4, 152,
    //     120, 159, 184, 101, 18, 239, 220, 197, 83, 151, 228, 188, 218,
    // ];

    // assert_eq!(&sighash[..], payload.as_slice());

    let r = <ethers_core::k256::Scalar as Reduce<Uint<4>>>::reduce_bytes(&big_r.x());
    let x_is_reduced = r.to_repr() != big_r.x();

    let v = RecoveryId::new(big_r.y_is_odd().into(), x_is_reduced);

    let signature = ethers_core::types::Signature {
        r: r.to_bytes().as_slice().into(),
        s: s.to_bytes().as_slice().into(),
        v: v.to_byte().into(),
    };

    println!("signature: {signature:?}");

    let signed_rlp_bytes = t.rlp_signed(&signature);

    let rlp = Rlp::new(&signed_rlp_bytes);

    println!("0x{:x}", signed_rlp_bytes);

    let res = TypedTransaction::decode_signed(&rlp).unwrap();

    println!("{res:?}");

    let sig2: ethers_core::types::Signature =
        MpcSignature(big_r_hex.to_string(), s_hex.to_string())
            .try_into()
            .unwrap();

    assert_eq!(sig2, signature);

    // let address =
    //     ethers_core::utils::parse_checksummed("0xC5acB93D901fb260359Cd1e982998236Cfac65E0", None)
    //         .unwrap();

    // signature.verify(payload, address).unwrap();
}

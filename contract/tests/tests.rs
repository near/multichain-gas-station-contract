// NOTE: If tests fail due to a directory not existing error, create `target/near/{contract,oracle,signer}`

use contract::{
    chain_configuration::PaymasterConfiguration,
    valid_transaction_request::ValidTransactionRequest, TransactionCreation,
};
use ethers_core::{
    k256::{
        ecdsa::RecoveryId,
        elliptic_curve::{
            bigint::Uint, group::GroupEncoding, ops::Reduce, point::AffineCoordinates,
            scalar::FromUintUnchecked, PrimeField,
        },
    },
    types::{transaction::eip2718::TypedTransaction, U256},
    utils::rlp::Rlp,
};
use lib::{foreign_address::ForeignAddress, signer::MpcSignature};
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
            let wasm = near_workspaces::compile_project("../mock/oracle")
                .await
                .unwrap();
            w.dev_deploy(&wasm).await.unwrap()
        },
        async {
            let wasm = near_workspaces::compile_project("../mock/signer")
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
            "balance": "100000000",
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
            minimum_available_balance: U256::from(100000000).0,
            key_path: "$".to_string()
        }]
    );

    let alice = w.dev_create_account().await.unwrap();

    let eth_transaction = ethers_core::types::transaction::eip1559::Eip1559TransactionRequest {
        chain_id: Some(0.into()),
        from: None,
        to: Some(ForeignAddress([1; 20]).into()),
        data: None,
        gas: Some(21000.into()),
        max_fee_per_gas: Some(100.into()),
        max_priority_fee_per_gas: Some(100.into()),
        access_list: vec![].into(),
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

    let alice_foreign_address = gas_station
        .view("get_foreign_address_for")
        .args_json(json!({
            "account_id": alice.id(),
        }))
        .await
        .unwrap()
        .json::<ForeignAddress>()
        .unwrap();

    let signed_transaction_bytes = hex::decode(signed_tx_2).unwrap();
    let signed_transaction_rlp = Rlp::new(&signed_transaction_bytes);
    let (tx, _s) = TypedTransaction::decode_signed(&signed_transaction_rlp).unwrap();
    assert_eq!(alice_foreign_address, tx.from().unwrap().into());
}

#[test]
#[ignore = "generate a payload signable by the contract"]
fn generate_eth_rlp_hex() {
    let eth_transaction = ethers_core::types::transaction::eip1559::Eip1559TransactionRequest {
        chain_id: Some(0.into()),
        from: None,
        to: Some(ForeignAddress([0x0f; 20]).into()),
        data: None,
        gas: Some(21000.into()),
        access_list: vec![].into(),
        max_fee_per_gas: Some(1234.into()),
        max_priority_fee_per_gas: Some(1234.into()),
        value: Some(1234.into()),
        nonce: Some(7777.into()),
    };

    println!("{}", hex::encode(eth_transaction.rlp()));
}

#[test]
fn decode_rlp() {
    // predicted address: 0x6D9BE8798fE027ea82f24d56b4Bea9B64BbBa54E
    // paymaster tx: 02f86a80808204d28204d2825208946d9be8798fe027ea82f24d56b4bea9b64bbba54e840316d52080c080a0f202ff2ce70dc105a881c782d68005b4260d8c31f42926b593e6632694214915a05b900840d0c04bcddceef7eb309751d048dc043160feaf0ae8ebde2ca6e151f8
    // user tx: 02f86a80821e618204d28204d2825208940f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f8204d280c080a0ddd9137ccc2b51220a51de20a0780f0fbff5c1cc715b29b11a500416b2f9e75da00edff5b1a1f02d4ce1937e024b7545f5a87b89b615cb2130bd87a890ba87358d

    let bytes = hex::decode(
        "02f86a80821e618204d28204d2825208940f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f8204d280c080a0ddd9137ccc2b51220a51de20a0780f0fbff5c1cc715b29b11a500416b2f9e75da00edff5b1a1f02d4ce1937e024b7545f5a87b89b615cb2130bd87a890ba87358d",
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
    let s = ethers_core::k256::Scalar::from_uint_unchecked(Uint::<4>::from_be_slice(
        &hex::decode(s_hex).unwrap(),
    ));

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

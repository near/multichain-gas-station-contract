// NOTE: If tests fail due to a directory not existing error, create `target/near/{oracle,signer}`

use contract::{
    chain_configuration::PaymasterConfiguration, foreign_address::ForeignAddress,
    signer_contract::MpcSignature, valid_transaction_request::ValidTransactionRequest,
    TransactionCreation,
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
    utils::rlp::{Decodable, Rlp},
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
    let tx = TypedTransaction::decode(&signed_transaction_rlp).unwrap();
    assert_eq!(alice_foreign_address, tx.from().unwrap().into());
}

#[test]
#[ignore = "generate a payload signable by the contract"]
fn generate_eth_rlp_hex() {
    let eth_transaction = ethers_core::types::TransactionRequest {
        chain_id: Some(97.into()),
        from: None,
        to: Some(ForeignAddress([0x0f; 20]).into()),
        data: None,
        gas: Some(21000.into()),
        gas_price: Some(8888.into()),
        value: Some(1234.into()),
        nonce: Some(7777.into()),
    };

    println!("{}", hex::encode(eth_transaction.rlp()));
}

#[test]
fn decode_rlp() {
    // ["f865118222b8825208941345301adbfb8d0ca0ddda64a68c4dfbdbd28e408416400b808080a0ee5aca1ea8216f98ff7743395b4caa21a8423146eb7e5e13e12385af124faf37a0bae88f6a08d0f042ddd3f68bfa659e75d4cda2ea6c7a9ca4b0511bc8fc4c2c34","f865821e618222b882520894abababababababababababababababababababab8204d28080a0b5aa8dafdc148ce9d6f9f12dc90abf11d1536db7f32a4b0b448d06024724bc84a016c61437de5c61c7f444943ac048917123bb92afa425f4b2c2da4ec1d9c8c907"]

    // first run:
    // paymaster tx from: 0x64cb2dee943db6b4a6a8f94c4e3eb81c44ca6c7f
    // paymaster tx to: 0x1345301adbfb8d0ca0ddda64a68c4dfbdbd28e40
    // user tx from: 0xf5c0d04504e796f2624cead901fae85c976ae8bd
    // public key: secp256k1:4HFcTSodRLVCGNVcGc4Mf2fwBBBxv9jxkGdiW2S2CA1y6UpVVRWKj6RX7d7TDt65k2Bj3w9FU4BGtt43ZvuhCnNt

    // second run:
    // first signed tx: f865128222b8825208941345301adbfb8d0ca0ddda64a68c4dfbdbd28e408416400b808001a0169f52f79fe6a7b322517882823c6caf64d463a35723c284e206f4e7ba9eb3d7a082104850ae0eb2407ec69df9cc4e977c33d1463a6c3f1f197a946e60be64981d
    // paymaster tx from: 0x286364ce3fe69909ecf201e8821280c26e413aea
    // paymaster tx to: 0x1345301adbfb8d0ca0ddda64a68c4dfbdbd28e40
    // second signed tx: f865821e618222b882520894ababababababababababababababababababadab8204d28080a0ca4c7d5b033891493f5494e078a59dddab4c358cf253308f19c9a37461678360a0820dd725e347a938695a9c0e4335895f4ceb9098249a5a8c10ee580121250022
    // user tx from: 0x812a6d0652e09f8df6a7754c6b1933a33ea42fc1

    // third run:
    // first signed tx: f865138222b8825208941345301adbfb8d0ca0ddda64a68c4dfbdbd28e408416400b808080a0ba93727e4d2dad388a90b04c3e7bf215ed90cd63fddb7247a8a5ea6cd81079c8a0250367f4c84ca019bfecd3bb2bf7bac9b4c9211425b0b1c553e63dd6592b5759
    // paymaster tx from: 0xc0a7a1648d2debf28cc29fef377a625b525f44bd
    // paymaster tx to: 0x1345301adbfb8d0ca0ddda64a68c4dfbdbd28e40
    // second signed tx: f865821e618222b882520894ababababababababababababababababababaeab8204d28080a00cfd42288ba70701554a0db9b93cc16f90b2440337d4867b1a14571825bc270aa0c2eaa1679b17fc5d66ddbaba5c7cd872dcd8349c4f5000071f31bfe4bdc9e41b
    // user tx from: 0x3ba43fc3d28621214161161a3794df820b31a7f6

    // fourth run (payload identical to third run):
    // first signed tx: f865138222b8825208941345301adbfb8d0ca0ddda64a68c4dfbdbd28e408416400b808001a0206e5d105365a448fa0eece7fff0b0b9c2284e921484601cfc74795a12082f3fa0ef066672f6e83d96eefc890c7164d6f683048127a2bc8243597b5fb3fd5e717c
    // paymaster tx from: 0x53929a94d243578da370c961832075f98cea2785
    // paymaster tx to: 0x1345301adbfb8d0ca0ddda64a68c4dfbdbd28e40
    // second signed tx: f865821e618222b882520894ababababababababababababababababababaeab8204d28080a068f8c262d95b98275eddd7ba2d8ff272919724681a9f53342d5b2dd066de20fca0d86d8655f90fcd29212d2eb18b22419cbf48692b18881e51d1b1eba46aea992e
    // user tx from: 0x7b7696549651f63f2e6b112acabd2cdc9ceb2796

    let bytes = hex::decode(
        "f865821e618222b882520894ababababababababababababababababababaeab8204d28080a068f8c262d95b98275eddd7ba2d8ff272919724681a9f53342d5b2dd066de20fca0d86d8655f90fcd29212d2eb18b22419cbf48692b18881e51d1b1eba46aea992e",
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

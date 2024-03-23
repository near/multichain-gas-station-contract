// NOTE: If tests fail due to a directory not existing error, create `target/near/{gas_station,oracle,signer}`

use ethers_core::{
    types::transaction::eip2718::TypedTransaction,
    utils::{self, hex, rlp::Rlp},
};
use gas_station::{
    chain_configuration::ViewPaymasterConfiguration, contract_event::TransactionSequenceSigned,
    Nep141ReceiverCreateTransactionArgs, TransactionSequenceCreation,
};
use lib::{
    asset::AssetId, foreign_address::ForeignAddress, kdf::get_mpc_address, nft_key::NftKeyMinted,
    signer::MpcSignature,
};
use near_sdk::{serde::Deserialize, serde_json::json};
use near_workspaces::{
    operations::Function,
    types::{Gas, NearToken},
};

#[tokio::test]
async fn test_workflow_happy_path() {
    let w = near_workspaces::sandbox().await.unwrap();

    let (gas_station, oracle, signer, nft_key, local_ft, alice) = tokio::join!(
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
        async {
            let wasm = near_workspaces::compile_project("../nft_key")
                .await
                .unwrap();
            w.dev_deploy(&wasm).await.unwrap()
        },
        async {
            let wasm = near_workspaces::compile_project("../mock/local_ft")
                .await
                .unwrap();
            let c = w.dev_deploy(&wasm).await.unwrap();
            c.call("new")
                .args_json(json!({}))
                .transact()
                .await
                .unwrap()
                .unwrap();
            c
        },
        async { w.dev_create_account().await.unwrap() },
    );

    println!("{:<16} {}", "Gas Station:", gas_station.id());
    println!("{:<16} {}", "Oracle:", oracle.id());
    println!("{:<16} {}", "Signer:", signer.id());
    println!("{:<16} {}", "NFT Key:", nft_key.id());
    println!("{:<16} {}", "Local FT:", local_ft.id());
    println!("{:<16} {}", "Alice:", alice.id());

    println!("Initializing the contracts...");

    println!("Initializing NFT key contract...");
    nft_key
        .call("new")
        .args_json(json!({
            "signer_contract_id": signer.id(),
        }))
        .max_gas()
        .transact()
        .await
        .unwrap()
        .unwrap();

    println!("Initializing gas station contract with Alice as owner...");
    alice
        .batch(gas_station.id())
        .call(Function::new("new").args_json(json!({
            "signer_contract_id": nft_key.id(),
            "oracle_id": oracle.id(),
            "supported_assets_oracle_asset_ids": [
                [AssetId::Native, "wrap.testnet"],
                [AssetId::Nep141(local_ft.id().as_str().parse().unwrap()), "local_ft.testnet"],
            ],
        })))
        .call(Function::new("add_foreign_chain").args_json(json!({
            "chain_id": "0",
            "oracle_asset_id": "weth.fakes.testnet",
            "transfer_gas": "21000",
            "fee_rate": ["120", "100"],
        })))
        .transact()
        .await
        .unwrap()
        .unwrap();

    println!("Performing storage deposits...");
    tokio::join!(
        async {
            alice
                .call(nft_key.id(), "storage_deposit")
                .args_json(json!({}))
                .deposit(NearToken::from_near(1))
                .transact()
                .await
                .unwrap()
                .unwrap();
        },
        async {
            alice
                .call(nft_key.id(), "storage_deposit")
                .args_json(json!({
                    "account_id": gas_station.id(),
                }))
                .deposit(NearToken::from_near(1))
                .transact()
                .await
                .unwrap()
                .unwrap();
        }
    );

    println!("Generating paymaster NFT key...");
    let paymaster_key = alice
        .call(nft_key.id(), "mint")
        .max_gas()
        .transact()
        .await
        .unwrap()
        .json::<NftKeyMinted>()
        .unwrap()
        .key_path;

    println!("Paymaster key: {paymaster_key}");

    println!("Transferring paymaster NFT key to gas station...");
    alice
        .call(nft_key.id(), "nft_transfer_call")
        .args_json(json!({
            "receiver_id": gas_station.id(),
            "token_id": paymaster_key,
            "msg": near_sdk::serde_json::to_string(&gas_station::Nep171ReceiverMsg {
                is_paymaster: true,
            }).unwrap(),
        }))
        .deposit(NearToken::from_yoctonear(1))
        .max_gas()
        .transact()
        .await
        .unwrap()
        .unwrap();

    println!("Adding paymaster...");
    alice
        .call(gas_station.id(), "add_paymaster")
        .args_json(json!({
            "chain_id": "0",
            "balance": "100000000",
            "nonce": 0,
            "key_path": paymaster_key,
        }))
        .transact()
        .await
        .unwrap()
        .unwrap();

    println!("Generating Alice's NFT key...");
    let alice_key = alice
        .call(nft_key.id(), "mint")
        .max_gas()
        .transact()
        .await
        .unwrap()
        .json::<NftKeyMinted>()
        .unwrap()
        .key_path;
    println!("Alice's NFT key: {alice_key}");

    println!("Transferring Alice's NFT key to gas station...");
    alice
        .call(nft_key.id(), "nft_transfer_call")
        .args_json(json!({
            "receiver_id": gas_station.id(),
            "token_id": alice_key,
            "msg": "",
        }))
        .deposit(NearToken::from_yoctonear(1))
        .max_gas()
        .transact()
        .await
        .unwrap()
        .unwrap();

    println!("Initialization complete.");

    println!("Checking paymaster configuration...");
    let result = gas_station
        .view("get_paymasters")
        .args_json(json!({
            "chain_id": "0",
        }))
        .await
        .unwrap()
        .json::<Vec<ViewPaymasterConfiguration>>()
        .unwrap();

    let result = &result[0];

    assert_eq!(result.nonce, 0);
    assert_eq!(
        result.minimum_available_balance,
        near_sdk::json_types::U128(100000000),
    );
    assert_eq!(result.key_path, paymaster_key);
    println!("Paymaster configuration check complete.");

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

    println!("Testing accepting deposits with NEP-141 token...");

    alice
        .call(local_ft.id(), "mint")
        .args_json(json!({
            "amount": near_sdk::json_types::U128(NearToken::from_near(10).as_yoctonear()),
        }))
        .transact()
        .await
        .unwrap()
        .unwrap();

    let res = alice
        .call(local_ft.id(), "ft_transfer_call")
        .args_json(json!({
            "receiver_id": gas_station.id(),
            "amount": near_sdk::json_types::U128(NearToken::from_near(1).as_yoctonear()),
            "msg": near_sdk::serde_json::to_string(&Nep141ReceiverCreateTransactionArgs {
                key_path: alice_key.clone(),
                transaction_rlp_hex: hex::encode_prefixed(&eth_transaction.rlp()),
                use_paymaster: Some(true),
            }).unwrap(),
        }))
        .deposit(NearToken::from_yoctonear(1))
        .max_gas()
        .transact()
        .await
        .unwrap();

    #[derive(Deserialize)]
    #[serde(crate = "near_sdk::serde")]
    struct Event {
        data: EventData,
    }

    #[derive(Deserialize)]
    #[serde(crate = "near_sdk::serde")]
    struct EventData {
        id: near_sdk::json_types::U64,
    }

    let id = res
        .logs()
        .into_iter()
        .find_map(|log| {
            log.strip_prefix("EVENT_JSON:")
                .and_then(|s| near_sdk::serde_json::from_str(s).ok())
        })
        .map(|e: Event| e.data.id)
        .unwrap();

    assert_eq!(id, 0.into(), "First transaction ID");

    println!("Done testing accepting deposits with NEP-141 token.");

    println!("Creating transaction...");

    let tx = alice
        .call(gas_station.id(), "create_transaction")
        .args_json(json!({
            "key_path": alice_key,
            "transaction_rlp_hex": hex::encode_prefixed(&eth_transaction.rlp()),
            "use_paymaster": true,
        }))
        .deposit(NearToken::from_near(1))
        .gas(Gas::from_tgas(50))
        .transact()
        .await
        .unwrap()
        .json::<TransactionSequenceCreation>()
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

    let _alice_foreign_address = gas_station
        .view("get_foreign_address_for")
        .args_json(json!({
            "account_id": alice.id(),
            "key_path": alice_key,
        }))
        .await
        .unwrap()
        .json::<ForeignAddress>()
        .unwrap();

    let signed_transaction_bytes = hex::decode(&signed_tx_2).unwrap();
    let signed_transaction_rlp = Rlp::new(&signed_transaction_bytes);
    let (_tx, _s) = TypedTransaction::decode_signed(&signed_transaction_rlp).unwrap();
    // IGNORE: due to not having a real MPC to mock and not actually deriving keys
    // assert_eq!(alice_foreign_address, tx.from().unwrap().into());

    let signed_transaction_sequences = gas_station
        .view("list_signed_transaction_sequences_after")
        .args_json(json!({
            "block_height": "0",
        }))
        .await
        .unwrap()
        .json::<Vec<TransactionSequenceSigned>>()
        .unwrap();

    assert_eq!(
        signed_transaction_sequences,
        vec![TransactionSequenceSigned {
            id: tx.id,
            foreign_chain_id: "0".to_string(),
            created_by_account_id: alice.id().as_str().parse().unwrap(),
            signed_transactions: vec![signed_tx_1, signed_tx_2],
        }]
    );

    println!("List of signed transactions:");
    println!("{:?}", signed_transaction_sequences);
}

#[test]
#[ignore = "generate a payload signable by the contract"]
fn generate_eth_rlp_hex() {
    let eth_transaction = ethers_core::types::transaction::eip1559::Eip1559TransactionRequest {
        chain_id: Some(97.into()),
        from: None,
        to: Some(ForeignAddress([0x0f; 20]).into()),
        data: None,
        gas: Some(21000.into()),
        access_list: vec![].into(),
        max_fee_per_gas: Some(1234.into()),
        max_priority_fee_per_gas: Some(1234.into()),
        value: Some(1234.into()),
        nonce: Some(8891.into()),
    };

    println!("RLP: {}", hex::encode_prefixed(eth_transaction.rlp()));
    let tx: TypedTransaction = eth_transaction.into();
    let mut sighash = tx.sighash().to_fixed_bytes();
    sighash.reverse();
    println!("Sighash: {:?}", sighash);
}

#[test]
fn decode_rlp() {
    // predicted address: 0x02d6ad0e6012a06ec7eb087cfcb10b8ce993b2c2
    // paymaster tx: 0x02f86a61018204d28204d28252089402d6ad0e6012a06ec7eb087cfcb10b8ce993b2c2840316d52080c080a0cc39fb05fcb8ade476f1230f8cdcab6959f46235d12df4b6a3ebd7ab8f2cce52a002c3883903979543780e68092fd4714ac7dbad71cd0b3451660d799ba40ffc9d
    // paymaster from: 0xd4ae9bbd30c1f55997aa308dedf1f3d01189bc2e
    // paymaster to: 0x02d6ad0e6012a06ec7eb087cfcb10b8ce993b2c2
    // user tx: 0x02f86a618222bb8204d28204d2825208940f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f8204d280c001a01e9f894cdcb789c70d959c44eaa8f2430856fb641e6712638635d25ca47c3cefa0514ac820e7228b6a07d849d614be54099f6cfa890d417924c830108448f8f995
    // user from: 0x02d6ad0e6012a06ec7eb087cfcb10b8ce993b2c2
    // user to: (junk)

    let bytes = hex::decode(
        "0x02f86a618222bb8204d28204d2825208940f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f8204d280c001a01e9f894cdcb789c70d959c44eaa8f2430856fb641e6712638635d25ca47c3cefa0514ac820e7228b6a07d849d614be54099f6cfa890d417924c830108448f8f995",
    )
    .unwrap();

    println!("{bytes:?}");

    let rlp = Rlp::new(&bytes);

    let txrq = TypedTransaction::decode_signed(&rlp).unwrap();

    println!("{txrq:?}");
}

#[test]
#[ignore]
fn test_derive_address() {
    let mpc_public_key = "secp256k1:4HFcTSodRLVCGNVcGc4Mf2fwBBBxv9jxkGdiW2S2CA1y6UpVVRWKj6RX7d7TDt65k2Bj3w9FU4BGtt43ZvuhCnNt".parse().unwrap();
    let a = get_mpc_address(mpc_public_key, &"hatchet.testnet".parse().unwrap(), "test").unwrap();
    println!("{a}");
}

#[test]
#[ignore]
fn test_derive_new_mpc() {
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
        nonce: Some(8891.into()),
    };
    let tx: TypedTransaction = eth_transaction.into();
    let sighash = tx.sighash().to_fixed_bytes();

    let mpc_signature = MpcSignature(
        "03DAE1E75B650ABC6AD22C899FC4245A9F58E323320B7380872C1813A7DCEB0F95".to_string(),
        "3FD2BC8430EC146E6D1B0EC64FE80EEDC0C483B95C8247FDFC5ADFC459BB3096".to_string(),
    );

    let sig: ethers_core::types::Signature = mpc_signature.try_into().unwrap();
    let recovered_address = sig.recover(sighash).unwrap();

    let signed_rlp_bytes = tx.rlp_signed(&sig);
    let signed_rlp = Rlp::new(&signed_rlp_bytes);
    let (recovered_signed_transaction, _decoded_sig) =
        TypedTransaction::decode_signed(&signed_rlp).unwrap();
    println!("{}", utils::to_checksum(&recovered_address, None));
    println!(
        "{}",
        utils::to_checksum(recovered_signed_transaction.from().unwrap(), None)
    );
    assert_eq!(
        &recovered_address,
        recovered_signed_transaction.from().unwrap()
    );
}

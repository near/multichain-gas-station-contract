// NOTE: If tests fail due to a directory not existing error, create `target/near/{contract,oracle,signer}`

use contract::{
    chain_configuration::ViewPaymasterConfiguration, contract_event::TransactionSequenceSigned,
    TransactionCreation,
};
use ethers_core::{
    types::transaction::eip2718::TypedTransaction,
    utils::{self, hex, rlp::Rlp},
};
use lib::{
    foreign_address::ForeignAddress,
    kdf::{derive_evm_address_for_account, get_mpc_address},
    signer::MpcSignature,
};
use near_sdk::serde_json::json;
use near_workspaces::{
    operations::Function,
    types::{Gas, NearToken},
};

#[tokio::test]
async fn test_workflow_happy_path() {
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
        .json::<Vec<ViewPaymasterConfiguration>>()
        .unwrap();

    let result = &result[0];

    assert_eq!(result.nonce, 0);
    assert_eq!(
        result.minimum_available_balance,
        near_sdk::json_types::U128(100000000),
    );
    assert_eq!(result.key_path, "$".to_string());

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
            "transaction_rlp_hex": hex::encode_prefixed(&eth_transaction.rlp()),
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

    let signed_transaction_bytes = hex::decode(&signed_tx_2).unwrap();
    let signed_transaction_rlp = Rlp::new(&signed_transaction_bytes);
    let (tx, _s) = TypedTransaction::decode_signed(&signed_transaction_rlp).unwrap();
    assert_eq!(alice_foreign_address, tx.from().unwrap().into());

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

    println!("RLP: {}", hex::encode_prefixed(eth_transaction.rlp()));
    let tx: TypedTransaction = eth_transaction.into();
    let mut sighash = tx.sighash().to_fixed_bytes();
    sighash.reverse();
    println!("Sighash: {:?}", sighash);
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
#[ignore]
fn test_derive_address() {
    let mpc_public_key = "secp256k1:4HFcTSodRLVCGNVcGc4Mf2fwBBBxv9jxkGdiW2S2CA1y6UpVVRWKj6RX7d7TDt65k2Bj3w9FU4BGtt43ZvuhCnNt".parse().unwrap();
    let a = get_mpc_address(mpc_public_key, &"hatchet.testnet".parse().unwrap(), "test").unwrap();
    println!("{a:?}");
    println!("{a}");
}

#[test]
#[ignore]
fn test_derive_new_mpc() {
    // derive key sha256 unreversed: 0x4f891037e68729357029A84b913a4a5Fa3E0F5bf
    // derive key sha256 reversed: 0x20c505Fe0E4Aa8dA8aF6437a99cF4B7DA0AfDE46

    // first signature from: 0x49ea71547Df3220814C2ca78583cCf0B661f6C5e

    // reversed first signature from: 0xb4C9b8A11A9a8D62b520F44CB34c9cC5Dcb112ad, then 0x4f891037e68729357029A84b913a4a5Fa3E0F5bf
    // reversed second signature from: 0x40D390cFFbA6D5255F855D4Ea14cfc1624dBFFeF, then 0x3a8bE0e31d9ACc969dF9cb2ecE935F331443B800
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
    let mut sighash = tx.sighash().to_fixed_bytes();
    // sighash.reverse();

    let mpc_signature = MpcSignature(
        // "029D401E23AA91038792D3C172E90D95728C8236917712BB33D5B36139554A9D88".to_string(),
        // "642D9CCABB7381AA80D7ADDE7AC01D51994E0315F02987EC1254C4CD15DAC5D3".to_string(),
        // "027509A493DF8B3643D85B6B4254AA27528B347D9E8B14AF56E66F420E494AFFC7".to_string(),
        // "4D2760E8B8D1898E2DB16DAB396832D7F52CC85751EDDC11F4C7625840A099FB".to_string(),
        //   "0333FF1E4FFA121E098D2EEF9A00EA28BF3D63E9D65628EE0B30F38A9F6DC89D11".to_string(),
        //   "01C4AC7FF297129B5A810B94BB638235D7EB733E0409EB2CC787BEE969CDF60C".to_string()
        // "023B077DBA817261A149BF1E28D4AA514A3426C717AE5E2419219E7F8F529AD2AC".to_string(),
        // "2523A131D418DEDC82EDB7E28E5D451C26DA34AFCD5EBCBF6DFB422A02B8F01D".to_string(),
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

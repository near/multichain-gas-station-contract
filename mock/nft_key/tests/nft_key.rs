use near_sdk::serde_json::json;

#[tokio::test]
async fn test_workflow_happy_path() {
    let w = near_workspaces::sandbox().await.unwrap();

    let (nft_key, signer) = tokio::join!(
        async {
            let wasm = near_workspaces::compile_project("./").await.unwrap();
            w.dev_deploy(&wasm).await.unwrap()
        },
        async {
            let wasm = near_workspaces::compile_project("../signer").await.unwrap();
            w.dev_deploy(&wasm).await.unwrap()
        },
    );

    println!("{:<16} {}", "NFT Key:", nft_key.id());
    println!("{:<16} {}", "Signer:", signer.id());

    println!("Initializing the contract...");

    nft_key
        .call("new")
        .args_json(json!({
            "signer_contract_id": signer.id(),
        }))
        .transact()
        .await
        .unwrap()
        .unwrap();

    println!("Initialization complete.");

    let (token_1_id, token_2_id) = tokio::join!(
        async {
            nft_key
                .call("mint")
                .args_json(json!({}))
                .transact()
                .await
                .unwrap()
                .json::<String>()
                .unwrap()
        },
        async {
            nft_key
                .call("mint")
                .args_json(json!({}))
                .transact()
                .await
                .unwrap()
                .json::<String>()
                .unwrap()
        },
    );

    let msg_1 = [1u8; 32];
    let msg_2 = [2u8; 32];

    let (sig_1, sig_2) = tokio::join!(
        async {
            nft_key
                .call("ck_sign_hash")
                .args_json(json!({
                    "path": token_1_id,
                    "payload": msg_1,
                }))
                .max_gas()
                .transact()
                .await
                .unwrap()
                .json::<String>()
                .unwrap()
        },
        async {
            nft_key
                .call("ck_sign_hash")
                .args_json(json!({
                    "path": token_2_id,
                    "payload": msg_2,
                }))
                .max_gas()
                .transact()
                .await
                .unwrap()
                .json::<String>()
                .unwrap()
        },
    );

    println!("sig1: {sig_1:?}");
    println!("sig2: {sig_2:?}");
}

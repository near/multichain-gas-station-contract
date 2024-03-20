use near_sdk::serde_json::json;
use near_sdk_contract_tools::nft::Token;
use near_workspaces::types::NearToken;

#[tokio::test]
async fn test_nft_key() {
    let w = near_workspaces::sandbox().await.unwrap();

    let (nft_key, signer, alice, bob) = tokio::join!(
        async {
            w.dev_deploy(&near_workspaces::compile_project("./").await.unwrap())
                .await
                .unwrap()
        },
        async {
            w.dev_deploy(&near_workspaces::compile_project("../mock/signer").await.unwrap())
                .await
                .unwrap()
        },
        async { w.dev_create_account().await.unwrap() },
        async { w.dev_create_account().await.unwrap() },
    );

    println!("{:<16} {}", "Alice:", alice.id());
    println!("{:<16} {}", "Bob:", bob.id());
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

    let token_1_id = alice
        .call(nft_key.id(), "mint")
        .args_json(json!({}))
        .transact()
        .await
        .unwrap()
        .json::<String>()
        .unwrap();
    let token_2_id = alice
        .call(nft_key.id(), "mint")
        .args_json(json!({}))
        .transact()
        .await
        .unwrap()
        .json::<String>()
        .unwrap();

    let msg_1 = [1u8; 32];
    let msg_2 = [2u8; 32];

    let (alice_success, bob_fail) = tokio::join!(
        async {
            alice
                .call(nft_key.id(), "ck_sign_hash")
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
            bob.call(nft_key.id(), "ck_sign_hash")
                .args_json(json!({
                    "path": token_2_id,
                    "payload": msg_2,
                }))
                .max_gas()
                .transact()
                .await
                .unwrap()
        },
    );

    println!("Alice successfully signed: {alice_success}");

    assert!(
        bob_fail.is_failure(),
        "Bob is unauthorized to sign with an NFT he does not own"
    );

    let (token_1, token_2) = tokio::join!(
        async {
            alice
                .view(nft_key.id(), "nft_token")
                .args_json(json!({ "token_id": token_1_id }))
                .await
                .unwrap()
                .json::<Token>()
                .unwrap()
        },
        async {
            alice
                .view(nft_key.id(), "nft_token")
                .args_json(json!({ "token_id": token_2_id }))
                .await
                .unwrap()
                .json::<Token>()
                .unwrap()
        },
    );

    println!("Token 1: {token_1:?}");
    println!("Token 2: {token_2:?}");

    println!("Transferring token {token_2_id} to Bob...");

    alice
        .call(nft_key.id(), "nft_transfer")
        .args_json(json!({
            "token_id": token_2_id,
            "receiver_id": bob.id(),
        }))
        .deposit(NearToken::from_yoctonear(1))
        .max_gas()
        .transact()
        .await
        .unwrap()
        .unwrap();

    println!("Token transferred.");
    println!("Bob attempting to sign with token {token_2_id}...");

    let bob_success = bob
        .call(nft_key.id(), "ck_sign_hash")
        .args_json(json!({
            "path": token_2_id,
            "payload": msg_2,
        }))
        .max_gas()
        .transact()
        .await
        .unwrap()
        .json::<String>()
        .unwrap();

    println!("After transfer, Bob signed: {bob_success}");

    println!("Approving Bob to sign with token {token_1_id} without transferring...");

    alice
        .call(nft_key.id(), "ck_approve")
        .args_json(json!({
            "path": token_1_id,
            "account_id": bob.id(),
        }))
        .max_gas()
        .transact()
        .await
        .unwrap()
        .unwrap();

    println!("Approval succeeded.");
    println!("Bob attempting to sign with token {token_1_id}...");

    let bob_is_approved = bob
        .call(nft_key.id(), "ck_sign_hash")
        .args_json(json!({
            "path": token_1_id,
            "payload": msg_1,
        }))
        .max_gas()
        .transact()
        .await
        .unwrap()
        .json::<String>()
        .unwrap();

    println!("After approval, Bob signed: {bob_is_approved}");

    println!("Revoking Bob's approval to sign with token {token_1_id}...");

    alice
        .call(nft_key.id(), "ck_revoke")
        .args_json(json!({
            "path": token_1_id,
            "account_id": bob.id(),
        }))
        .max_gas()
        .transact()
        .await
        .unwrap()
        .unwrap();

    println!("Revokation succeeded.");
    println!("Bob attempting to sign with token {token_1_id}...");

    let bob_is_no_longer_approved = bob
        .call(nft_key.id(), "ck_sign_hash")
        .args_json(json!({
            "path": token_1_id,
            "payload": msg_1,
        }))
        .max_gas()
        .transact()
        .await
        .unwrap();

    assert!(
        bob_is_no_longer_approved.is_failure(),
        "Bob is no longer approved to sign with token {token_1_id}",
    );

    println!("Bob failed to sign with revoked key.");
}

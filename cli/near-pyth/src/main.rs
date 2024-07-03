#![allow(clippy::too_many_lines)]

use clap::{Parser, Subcommand};
use futures_util::StreamExt;
use lib::pyth::PriceIdentifier;
use near_crypto::InMemorySigner;
use near_fetch::{result::ExecutionFinalResult, signer::SignerExt};
use near_jsonrpc_client::{
    NEAR_MAINNET_ARCHIVAL_RPC_URL, NEAR_MAINNET_RPC_URL, NEAR_TESTNET_ARCHIVAL_RPC_URL,
    NEAR_TESTNET_RPC_URL,
};
use near_primitives::types::AccountId;
use near_token::NearToken;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    fmt::Display,
    path::{Path, PathBuf},
    str::FromStr,
};
use tokio::sync::mpsc;

const USER_AGENT: &str = concat!("near-pyth/", env!("CARGO_PKG_VERSION"));

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long, default_value = "https://hermes-beta.pyth.network")]
    endpoint: Url,

    #[arg(long, short)]
    contract_id: Option<AccountId>,

    #[arg(long, short, default_value_t = default_network())]
    network: String,

    #[arg(long, short, default_value_t = NearToken::from_millinear(100))]
    max_fee: NearToken,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Find {
        query: String,
    },
    Get {
        price_ids: Vec<String>,

        #[arg(long, group = "format")]
        json: bool,
    },
    ContractGet {
        price_ids: Vec<String>,
    },
    Update {
        price_ids: Vec<String>,

        #[arg(long, short)]
        key_file: PathBuf,
    },
    StreamUpdate {
        price_ids: Vec<String>,

        #[arg(long, short)]
        key_file: PathBuf,
    },
}

fn default_network() -> String {
    std::env::var("NEAR_ENV")
        .ok()
        .unwrap_or_else(|| "testnet".to_string())
}

fn near_rpc_resolver(s: &str) -> &str {
    match &s.to_lowercase()[..] {
        "mainnet" => NEAR_MAINNET_RPC_URL,
        "testnet" => NEAR_TESTNET_RPC_URL,
        _ => s,
    }
}

async fn resolve_ids(
    client: &reqwest::Client,
    endpoint: &Url,
    price_ids: impl IntoIterator<Item = impl AsRef<str>>,
) -> Vec<PriceIdentifier> {
    let mut resolved_ids = Vec::new();

    for price_id in price_ids {
        resolved_ids.push(resolve_price_id(client, endpoint, price_id.as_ref()).await);
    }

    resolved_ids
}

async fn get_prices_http(
    client: &reqwest::Client,
    endpoint: &Url,
    price_ids: impl IntoIterator<Item = impl AsRef<str>>,
) -> PriceResponse {
    let query_ids = resolve_ids(client, endpoint, price_ids)
        .await
        .into_iter()
        .map(|id| ("ids[]", id))
        .collect::<Vec<_>>();

    client
        .get(endpoint.join("/v2/updates/price/latest").unwrap())
        .query(&[("encoding", "hex"), ("parsed", "true")])
        .query(&query_ids)
        .send()
        .await
        .unwrap()
        .json::<PriceResponse>()
        .await
        .unwrap()
}

async fn get_price_onchain(
    near: &near_fetch::Client,
    contract_id: &AccountId,
    price_id: PriceIdentifier,
) -> PythPrice {
    near.view(contract_id, "get_price")
        .args_json(json!({
            "price_identifier": price_id,
        }))
        .await
        .unwrap()
        .json::<PythPrice>()
        .unwrap()
}

async fn find_feeds(
    client: &reqwest::Client,
    endpoint: &Url,
    query: &str,
) -> Vec<PythFeedDescription> {
    client
        .get(endpoint.join("/v2/price_feeds").unwrap())
        .query(&[("query", query)])
        .send()
        .await
        .unwrap()
        .json::<Vec<PythFeedDescription>>()
        .await
        .unwrap()
}

fn default_pyth_contract_id(network_rpc: &str) -> Option<AccountId> {
    if network_rpc == NEAR_MAINNET_RPC_URL || network_rpc == NEAR_MAINNET_ARCHIVAL_RPC_URL {
        Some("pyth-oracle.near".parse().unwrap())
    } else if network_rpc == NEAR_TESTNET_RPC_URL || network_rpc == NEAR_TESTNET_ARCHIVAL_RPC_URL {
        Some("pyth-oracle.testnet".parse().unwrap())
    } else {
        None
    }
}

async fn push_update_to_chain(
    near: &near_fetch::Client,
    signer: &dyn SignerExt,
    contract_id: &AccountId,
    vaa: &str,
    max_fee: &NearToken,
) -> ExecutionFinalResult {
    let fee = near
        .view(contract_id, "get_update_fee_estimate")
        .args_json(json!({
            "data": vaa,
        }))
        .await
        .unwrap()
        .json::<NearToken>()
        .unwrap();

    assert!(&fee <= max_fee, "Quoted fee exceeds max: {fee} > {max_fee}");

    near.call(signer, contract_id, "update_price_feeds")
        .args_json(json!({
            "data": vaa,
        }))
        .deposit(fee)
        .max_gas()
        .transact()
        .await
        .unwrap()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KeyFile {
    private_key: String,
    account_id: AccountId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PriceResponse {
    binary: BinaryVaa,
    parsed: Vec<PythPriceFeed>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BinaryVaa {
    encoding: String,
    data: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PythFeedDescription {
    id: lib::pyth::PriceIdentifier,
    attributes: PythFeedDescriptionAttributes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PythFeedDescriptionAttributes {
    asset_type: String,
    base: String,
    description: String,
    generic_symbol: Option<String>,
    quote_currency: String,
    symbol: String,
    weekly_schedule: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PythPriceFeed {
    id: PriceIdentifier,
    price: PythPrice,
    ema_price: PythPrice,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PythPrice {
    price: String,
    conf: String,
    expo: i32,
    publish_time: i64,
}

impl Display for PythPrice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let time = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(self.publish_time * 1000)
            .unwrap();

        let now = chrono::Utc::now();
        let delta = time - now;

        let time_str = time.with_timezone(&chrono::Local).to_rfc3339();

        let expo_factor = 10f64.powi(self.expo);

        let mut price = f64::from_str(&self.price).unwrap();
        price *= expo_factor;

        let mut conf = f64::from_str(&self.conf).unwrap();
        conf *= expo_factor;

        let human_delta = chrono_humanize::HumanTime::from(delta);

        write!(f, "{price:.2} ± {conf:.2} @ {time_str} ({human_delta})")
    }
}

async fn resolve_price_id(
    client: &reqwest::Client,
    endpoint: &Url,
    s: &str,
) -> lib::pyth::PriceIdentifier {
    let raw = const_hex::decode_to_array(s).ok().or_else(|| {
        bs58::decode(s)
            .into_vec()
            .ok()
            .and_then(|v| v.try_into().ok())
    });

    if let Some(raw) = raw {
        lib::pyth::PriceIdentifier(raw)
    } else {
        find_feeds(client, endpoint, s).await[0].id
    }
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();

    let http = reqwest::ClientBuilder::new()
        .user_agent(USER_AGENT)
        .build()
        .unwrap();

    let network_rpc = near_rpc_resolver(&args.network);

    let near = near_fetch::Client::new(network_rpc);

    let contract_id = args
        .contract_id
        .clone()
        .or_else(|| default_pyth_contract_id(network_rpc))
        .expect("Could not determine Pyth contract ID");

    match args.command {
        Command::StreamUpdate {
            price_ids,
            key_file,
        } => {
            let resolved = resolve_ids(&http, &args.endpoint, price_ids).await;

            let signer = get_signer_from_key_file(&key_file);

            let (send, mut recv) = mpsc::unbounded_channel::<String>();

            let mut url = args.endpoint.join("/v2/updates/price/stream").unwrap();
            let mut params = url.query_pairs_mut();
            for id in resolved {
                params.append_pair("ids[]", &id.to_string());
            }
            params.append_pair("encoding", "hex");
            params.append_pair("parsed", "true");
            params.append_pair("allow_unordered", "true");
            params.append_pair("benchmarks_only", "false");
            drop(params);

            let mut es = reqwest_eventsource::EventSource::get(url);

            tokio::spawn({
                async move {
                    loop {
                        let mut msgs = Vec::with_capacity(recv.len());

                        recv.recv_many(&mut msgs, recv.len()).await;

                        if let Some(newest_vaa) = msgs.last() {
                            println!("Skipping {}, pushing newest data only", msgs.len() - 1);
                            let res = push_update_to_chain(
                                &near,
                                &signer,
                                &contract_id,
                                newest_vaa,
                                &args.max_fee,
                            )
                            .await
                            .unwrap();
                            println!("Update TXID: {}", res.details.transaction.hash);
                        }
                    }
                }
            });

            while let Some(event) = es.next().await {
                match event {
                    Ok(reqwest_eventsource::Event::Open) => {}
                    Ok(reqwest_eventsource::Event::Message(msg)) => {
                        let Ok(response) = serde_json::from_str::<PriceResponse>(&msg.data) else {
                            continue;
                        };

                        println!("Got: {:?}", response.parsed);

                        send.send(response.binary.data[0].clone()).unwrap();
                    }
                    Err(e) => {
                        eprintln!("Error: {e}");
                        es.close();
                        break;
                    }
                }
            }

            todo!()
        }
        Command::ContractGet { price_ids } => {
            let resolved = resolve_ids(&http, &args.endpoint, price_ids).await;

            for id in resolved {
                let price = get_price_onchain(&near, &contract_id, id).await;
                println!("{id}: {price}");
            }
        }
        Command::Update {
            price_ids,
            key_file,
        } => {
            let response = get_prices_http(&http, &args.endpoint, price_ids).await;
            let vaa = &response.binary.data[0];

            let signer = get_signer_from_key_file(&key_file);

            println!("Acting account: {}", signer.account_id);

            let result = push_update_to_chain(&near, &signer, &contract_id, vaa, &args.max_fee)
                .await
                .unwrap();

            println!("TXID: {}", result.details.transaction.hash);
        }
        Command::Find { query } => {
            let result = find_feeds(&http, &args.endpoint, &query).await;

            for feed in result {
                println!("{}", feed.id);
                println!("\t{}", feed.attributes.symbol);
                println!("\t{}", feed.attributes.description);
                println!();
            }
        }
        Command::Get { price_ids, json } => {
            let feeds = get_prices_http(&http, &args.endpoint, price_ids).await;

            if json {
                println!("{}", serde_json::to_string(&feeds.parsed).unwrap());
            } else {
                for feed in feeds.parsed {
                    println!("Feed ID: {}", feed.id);

                    println!("{}", feed.price);
                }
            }
        }
    }
}

fn get_signer_from_key_file(key_file: &Path) -> InMemorySigner {
    let key_file = std::fs::read_to_string(key_file).unwrap();
    let KeyFile {
        account_id,
        private_key,
    } = serde_json::from_str::<KeyFile>(&key_file).unwrap();
    InMemorySigner::from_secret_key(account_id.clone(), private_key.parse().unwrap())
}

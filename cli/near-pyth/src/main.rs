use clap::{Parser, Subcommand};
use lib::pyth::PriceIdentifier;
use near_primitives::{types::AccountId, validator_signer::InMemoryValidatorSigner};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, str::FromStr};

const USER_AGENT: &str = concat!("near-pyth/", env!("CARGO_PKG_VERSION"));

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long, default_value = "https://hermes-beta.pyth.network")]
    endpoint: Url,

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
    Update {
        price_ids: Vec<String>,

        #[arg(long, short)]
        key_file: PathBuf,

        #[arg(long, short)]
        contract_id: AccountId,

        #[arg(long, short, default_value_t = default_network())]
        network: String,
    },
}

fn default_network() -> String {
    std::env::var("NEAR_ENV")
        .ok()
        .unwrap_or_else(|| "testnet".to_string())
}

fn near_rpc_resolver(s: &str) -> Url {
    match s {
        "mainnet" => Url::parse(near_jsonrpc_client::NEAR_MAINNET_RPC_URL).unwrap(),
        "testnet" => Url::parse(near_jsonrpc_client::NEAR_TESTNET_RPC_URL).unwrap(),
        _ => Url::parse(s).unwrap(),
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

async fn get_price(
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

impl Command {
    pub async fn exec(
        &self,
        out: &mut impl std::io::Write,
        client: &reqwest::Client,
        endpoint: &Url,
    ) -> std::io::Result<()> {
        match self {
            Command::Update {
                price_ids,
                key_file,
                network,
                contract_id,
            } => {
                // let mut query_price_ids = Vec::new();
                let key_file = std::fs::read_to_string(key_file)?;
                let KeyFile {
                    account_id,
                    private_key,
                } = serde_json::from_str::<KeyFile>(&key_file)?;
                let signer = near_crypto::InMemorySigner::from_secret_key(
                    account_id.clone(),
                    private_key.parse().unwrap(),
                );

                let near = near_fetch::Client::new(near_rpc_resolver(network));

                near.call(&signer, , function)

                todo!()
            }
            Command::Find { query } => {
                let result = find_feeds(client, endpoint, query).await;

                for feed in result {
                    writeln!(out, "{}", feed.id)?;
                    writeln!(out, "\t{}", feed.attributes.symbol)?;
                    writeln!(out, "\t{}", feed.attributes.description)?;
                    writeln!(out)?;
                }

                Ok(())
            }
            Command::Get { price_ids, json } => {
                let feeds = get_price(client, endpoint, price_ids).await;

                if *json {
                    writeln!(out, "{}", serde_json::to_string(&feeds.parsed).unwrap())
                } else {
                    for feed in feeds.parsed {
                        writeln!(out, "Feed ID: {}", feed.id)?;

                        let time = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(
                            feed.price.publish_time * 1000,
                        )
                        .unwrap();

                        let now = chrono::Utc::now();
                        let delta = now - time;

                        let time_str = time.with_timezone(&chrono::Local).to_rfc3339();

                        let expo_factor = 10f64.powi(feed.price.expo);

                        let mut price = f64::from_str(&feed.price.price).unwrap();
                        price *= expo_factor;

                        let mut conf = f64::from_str(&feed.price.conf).unwrap();
                        conf *= expo_factor;

                        let human_delta = chrono_humanize::HumanTime::from(delta);

                        writeln!(out, "{price:.2} ± {conf:.2} @ {time_str} ({human_delta})")?;
                    }

                    Ok(())
                }
            }
        }
    }
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
    let client = reqwest::ClientBuilder::new()
        .user_agent(USER_AGENT)
        .build()
        .unwrap();

    args.command
        .exec(&mut std::io::stdout(), &client, &args.endpoint)
        .await
        .unwrap();
}

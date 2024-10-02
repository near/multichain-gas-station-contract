use clap::{Parser, Subcommand};
use lib::pyth::PriceIdentifier;
use near_crypto::InMemorySigner;
use near_primitives::types::AccountId;
use near_token::NearToken;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Display,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

mod app;
use app::App;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Pyth API endpoint. If unspecified, the default for the network will be
    /// used, if known.
    #[arg(short, long)]
    endpoint: Option<Url>,

    /// Pyth oracle contract ID. If unspecified, the default for the network
    /// will be used, if known.
    #[arg(long, short)]
    contract_id: Option<AccountId>,

    /// NEAR RPC to use. Specify one of "mainnet", "testnet", or a URL.
    #[arg(long, short, default_value_t = default_network())]
    network: String,

    /// Maximum fee to pay each time an update is pushed.
    #[arg(long, short = 'f', default_value_t = NearToken::from_millinear(25))]
    max_fee: NearToken,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Search for a feed.
    Find { query: String },
    /// Get prices for a list of feeds from the Pyth HTTP endpoint.
    HttpGet {
        queries: Vec<String>,

        /// Output raw JSON.
        #[arg(long, group = "format")]
        json: bool,
    },
    /// Get prices for a list of feeds from the Pyth oracle contract on NEAR.
    ContractGet { queries: Vec<String> },
    /// Push a single price update to the Pyth oracle contract for each of the
    /// queried feeds.
    Update {
        queries: Vec<String>,

        /// Path to the key file to use for signing.
        #[arg(long, short)]
        key_file: PathBuf,
    },
    /// Continuously push price updates to the Pyth oracle contract for each of
    /// the queried feeds.
    StreamUpdate {
        queries: Vec<String>,

        /// Path to the key file to use for signing.
        #[arg(long, short)]
        key_file: PathBuf,
    },
}

fn default_network() -> String {
    std::env::var("NEAR_ENV")
        .ok()
        .unwrap_or_else(|| "testnet".to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KeyFile {
    private_key: String,
    account_id: AccountId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PriceResponse {
    binary: BinaryData,
    parsed: Vec<PythPriceFeed>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BinaryData {
    encoding: String,
    data: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PythFeedDescription {
    id: PriceIdentifier,
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
    weekly_schedule: Option<String>,
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

#[tokio::main]
async fn main() {
    let args = Cli::parse();

    let app = App::new(&args.network)
        .with_contract(args.contract_id)
        .with_endpoint(args.endpoint);

    match args.command {
        Command::StreamUpdate { queries, key_file } => {
            let price_ids = app.resolve_price_ids(queries).await;

            let signer = get_signer_from_key_file(&key_file);

            println!("Acting account: {}", signer.account_id);

            Arc::new(app)
                .stream_update(Arc::new(signer), &price_ids, args.max_fee)
                .await;
        }
        Command::ContractGet { queries } => {
            let price_ids = app.resolve_price_ids(queries).await;

            for id in price_ids {
                let price = app.get_onchain_price(id).await;
                println!("Feed ID: {id}");
                if let Some(price) = price {
                    println!("{price}");
                } else {
                    println!("No price found");
                }
                println!();
            }
        }
        Command::Update { queries, key_file } => {
            let price_ids = app.resolve_price_ids(queries).await;
            let response = app.get_http_prices(&price_ids).await;
            let vaa = &response.binary.data[0];

            let signer = get_signer_from_key_file(&key_file);

            println!("Acting account: {}", signer.account_id);

            let result = app
                .push_update_to_chain(&signer, vaa, &args.max_fee)
                .await
                .unwrap();

            println!("TXID: {}", result.details.transaction.hash);
        }
        Command::Find { query } => {
            let result = app.find_feeds(&query).await;

            for feed in result {
                println!("{}", feed.id);
                println!("\t{}", feed.attributes.symbol);
                println!("\t{}", feed.attributes.description);
                println!();
            }
        }
        Command::HttpGet { queries, json } => {
            let price_ids = app.resolve_price_ids(queries).await;
            let feeds = app.get_http_prices(&price_ids).await;

            if json {
                println!("{}", serde_json::to_string(&feeds.parsed).unwrap());
            } else {
                for feed in feeds.parsed {
                    println!("Feed ID: {}", feed.id);
                    println!("{}", feed.price);
                    println!();
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

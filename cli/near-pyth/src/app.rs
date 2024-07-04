use futures_util::StreamExt;
use lib::pyth::PriceIdentifier;
use near_fetch::{result::ExecutionFinalResult, signer::SignerExt};
use near_jsonrpc_client::{
    NEAR_MAINNET_ARCHIVAL_RPC_URL, NEAR_MAINNET_RPC_URL, NEAR_TESTNET_ARCHIVAL_RPC_URL,
    NEAR_TESTNET_RPC_URL,
};
use near_primitives::types::AccountId;
use near_token::NearToken;
use reqwest::Url;
use serde_json::json;
use tokio::sync::mpsc;

use crate::{PriceResponse, PythFeedDescription, PythPrice};

const USER_AGENT: &str = concat!("near-pyth/", env!("CARGO_PKG_VERSION"));

pub struct App {
    pub http: reqwest::Client,
    pub endpoint: Url,
    pub near: near_fetch::Client,
    pub contract_id: AccountId,
}

impl App {
    pub fn new(near_network: &str) -> Self {
        let http = reqwest::ClientBuilder::new()
            .user_agent(USER_AGENT)
            .build()
            .unwrap();

        let rpc_url = match &near_network.to_lowercase()[..] {
            "mainnet" => NEAR_MAINNET_RPC_URL,
            "testnet" => NEAR_TESTNET_RPC_URL,
            "mainnet-archival" => NEAR_MAINNET_ARCHIVAL_RPC_URL,
            "testnet-archival" => NEAR_TESTNET_ARCHIVAL_RPC_URL,
            _ => near_network,
        };

        let (pyth_contract, pyth_endpoint) =
            if rpc_url == NEAR_MAINNET_RPC_URL || rpc_url == NEAR_MAINNET_ARCHIVAL_RPC_URL {
                ("pyth-oracle.near", "https://hermes.pyth.network")
            } else {
                ("pyth-oracle.testnet", "https://hermes-beta.pyth.network")
            };

        let near = near_fetch::Client::new(rpc_url);

        Self {
            http,
            endpoint: Url::parse(pyth_endpoint).unwrap(),
            near,
            contract_id: pyth_contract.parse().unwrap(),
        }
    }

    pub fn with_contract(self, contract_id: Option<AccountId>) -> Self {
        Self {
            contract_id: contract_id.unwrap_or(self.contract_id),
            ..self
        }
    }

    pub fn with_endpoint(self, endpoint: Option<Url>) -> Self {
        Self {
            endpoint: endpoint.unwrap_or(self.endpoint),
            ..self
        }
    }

    pub async fn find_feeds(&self, query: &str) -> Vec<PythFeedDescription> {
        self.http
            .get(self.endpoint.join("/v2/price_feeds").unwrap())
            .query(&[("query", query)])
            .send()
            .await
            .unwrap()
            .json::<Vec<PythFeedDescription>>()
            .await
            .unwrap()
    }

    pub async fn resolve_price_id(&self, s: &str) -> PriceIdentifier {
        let raw = const_hex::decode_to_array(s).ok().or_else(|| {
            bs58::decode(s)
                .into_vec()
                .ok()
                .and_then(|v| v.try_into().ok())
        });

        if let Some(raw) = raw {
            PriceIdentifier(raw)
        } else {
            self.find_feeds(s).await[0].id
        }
    }

    pub async fn resolve_price_ids(
        &self,
        queries: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Vec<PriceIdentifier> {
        let mut resolved_ids = Vec::new();

        for price_id in queries {
            resolved_ids.push(self.resolve_price_id(price_id.as_ref()).await);
        }

        resolved_ids
    }

    pub async fn get_http_prices(&self, price_ids: &[PriceIdentifier]) -> PriceResponse {
        let query_ids = price_ids.iter().map(|id| ("ids[]", id)).collect::<Vec<_>>();

        self.http
            .get(self.endpoint.join("/v2/updates/price/latest").unwrap())
            .query(&[("encoding", "hex"), ("parsed", "true")])
            .query(&query_ids)
            .send()
            .await
            .unwrap()
            .json::<PriceResponse>()
            .await
            .unwrap()
    }

    pub async fn get_onchain_price(&self, price_id: PriceIdentifier) -> PythPrice {
        self.near
            .view(&self.contract_id, "get_price")
            .args_json(json!({
                "price_identifier": price_id,
            }))
            .await
            .unwrap()
            .json::<PythPrice>()
            .unwrap()
    }

    pub async fn push_update_to_chain(
        &self,
        signer: &dyn SignerExt,
        data: &str,
        max_fee: &NearToken,
    ) -> ExecutionFinalResult {
        let fee = self
            .near
            .view(&self.contract_id, "get_update_fee_estimate")
            .args_json(json!({
                "data": data,
            }))
            .await
            .unwrap()
            .json::<NearToken>()
            .unwrap();

        assert!(&fee <= max_fee, "Quoted fee exceeds max: {fee} > {max_fee}");

        self.near
            .call(signer, &self.contract_id, "update_price_feeds")
            .args_json(json!({
                "data": data,
            }))
            .deposit(fee)
            .max_gas()
            .transact()
            .await
            .unwrap()
    }

    pub async fn stream_update(
        &self,
        signer: &dyn SignerExt,
        price_ids: &[PriceIdentifier],
        max_fee: NearToken,
    ) -> ! {
        let (send, mut recv) = mpsc::unbounded_channel::<String>();

        let mut url = self.endpoint.join("/v2/updates/price/stream").unwrap();
        let mut params = url.query_pairs_mut();
        for id in price_ids {
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
                while let Some(event) = es.next().await {
                    match event {
                        Ok(reqwest_eventsource::Event::Open) => {}
                        Ok(reqwest_eventsource::Event::Message(msg)) => {
                            let Ok(response) = serde_json::from_str::<PriceResponse>(&msg.data)
                            else {
                                continue;
                            };

                            println!("{}: {}", response.parsed[0].id, response.parsed[0].price);

                            send.send(response.binary.data[0].clone()).unwrap();
                        }
                        Err(e) => {
                            eprintln!("Error: {e}");
                            es.close();
                            break;
                        }
                    }
                }
            }
        });

        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            let mut msgs = Vec::with_capacity(recv.len());

            recv.recv_many(&mut msgs, recv.len()).await;

            if let Some(newest_vaa) = msgs.last() {
                println!("Skipping {}, pushing newest data only", msgs.len() - 1);
                let res = self
                    .push_update_to_chain(signer, newest_vaa, &max_fee)
                    .await
                    .unwrap();
                println!("TXID: {}", res.details.transaction.hash);
            }
        }
    }
}

use collector_rust::{
    config::{load_config, Config},
    crypto::{HexStringOrBuffer, ShardusCrypto},
    storage::Storage,
    types::{
        Account, Cycle, OriginalTxData, Receipt, Transaction,
    },
};
use futures_util::StreamExt;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

// Define specific response types for the distributor API
#[derive(Deserialize, Debug)]
struct TotalDataResponse {
    #[serde(rename = "totalReceipts")]
    total_receipts: i64,
    #[serde(rename = "totalCycles")]
    total_cycles: i64,
    #[serde(rename = "totalOriginalTxs")]
    total_original_txs: i64,
    #[serde(rename = "totalAccounts")]
    total_accounts: Option<i64>, // optional
    #[serde(rename = "totalTransactions")]
    total_transactions: Option<i64>, // optional
}

#[derive(Deserialize, Debug)]
struct CycleResponse {
    #[serde(rename = "cycleInfo")]
    cycle_info: Vec<Cycle>,
}

#[derive(Deserialize, Debug)]
struct ReceiptResponse {
    receipts: Vec<Receipt>,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct ReceiptCountResponse {
    receipts: Vec<ReceiptCount>,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct ReceiptCount {
    cycle: i64,
    receipts: i64,
}


#[derive(Deserialize, Debug)]
struct OriginalTxResponse {
    #[serde(rename = "originalTxs")]
    original_txs: Vec<OriginalTxData>,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct OriginalTxCountResponse {
    #[serde(rename = "originalTxs")]
    original_txs: Vec<OriginalTxCount>,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct OriginalTxCount {
    cycle: i64,
    #[serde(rename = "originalTxsData")]
    original_txs_data: i64,
}

#[derive(Deserialize, Debug)]
struct AccountResponse {
    accounts: Vec<Account>,
    #[serde(rename = "totalAccounts")]
    #[allow(dead_code)]
    total_accounts: Option<i64>,
}

#[derive(Deserialize, Debug)]
struct TransactionResponse {
    transactions: Vec<Transaction>,
    #[serde(rename = "totalTransactions")]
    #[allow(dead_code)]
    total_transactions: Option<i64>,
}


#[derive(Serialize)]
struct QueryParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    start: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<i64>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    query_type: Option<String>,
    #[serde(rename = "startCycle", skip_serializing_if = "Option::is_none")]
    start_cycle: Option<i64>,
    #[serde(rename = "endCycle", skip_serializing_if = "Option::is_none")]
    end_cycle: Option<i64>,

    sender: String,
    sign: Option<String>,
}

enum DataType {
    Cycle,
    Receipt,
    OriginalTx,
    Account,
    Transaction,
    TotalData,
}

impl DataType {
    fn endpoint(&self) -> &'static str {
        match self {
            DataType::Cycle => "/cycleinfo",
            DataType::Receipt => "/receipt",
            DataType::OriginalTx => "/originalTx",
            DataType::Account => "/account",
            DataType::Transaction => "/transaction",
            DataType::TotalData => "/totalData",
        }
    }
}

struct Collector {
    config: Config,
    storage: Storage,
    crypto: ShardusCrypto,
    client: reqwest::Client,
}

impl Collector {
    fn new(config: Config, storage: Storage) -> Self {
        let crypto = ShardusCrypto::new(&config.hash_key);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(45))
            .build()
            .unwrap();

        Self {
            config,
            storage,
            crypto,
            client,
        }
    }

    // Helper to sign and query
    async fn query<T: for<'de> Deserialize<'de>>(
        &self,
        data_type: DataType,
        mut params: QueryParams,
    ) -> Result<Option<T>, Box<dyn std::error::Error>> {
        let url = format!("http://{}:{}{}",
            self.config.distributor_info.ip,
            self.config.distributor_info.port,
            data_type.endpoint()
        );

        // Sign parameters
        params.sender = self.config.collector_info.public_key.clone();

        // Create a Value map to sign
        let mut map = serde_json::to_value(&params)?;
        if let Value::Object(ref mut m) = map {
            m.remove("sign"); // Ensure sign is undefined/null before signing
        }

        let msg_string = serde_json::to_string(&map)?;
        let kp = self.crypto.get_key_pair_using_sk(&HexStringOrBuffer::Hex(self.config.collector_info.secret_key.clone()));

        // Sign
        let sig = self.crypto.sign(HexStringOrBuffer::Hex(msg_string), &kp.secret_key)?;
        let sig_hex = hex::encode(sig);

        params.sign = Some(sig_hex);

        // Send Request
        let res = self.client.post(&url)
            .json(&params)
            .send()
            .await?;

        if res.status() == StatusCode::OK {
             let text = res.text().await?;
             if text.is_empty() {
                 return Ok(None);
             }
             let val: Value = serde_json::from_str(&text)?;
             let parsed: T = serde_json::from_value(val)?;
             Ok(Some(parsed))
        } else {
            tracing::warn!("Query failed: {} {}", url, res.status());
            Ok(None)
        }
    }

    async fn sync_data(&self) -> Result<(), Box<dyn std::error::Error>> {
        // 1. Get Local Counts
        let mut last_receipt_count = self.storage.get_receipt_count().await?;
        let mut last_original_tx_count = self.storage.get_original_tx_data_count().await?;
        let mut last_cycle_count = self.storage.get_cycle_count().await?;

        // 2. Get Remote Counts
        let total_data = self.query::<TotalDataResponse>(DataType::TotalData, QueryParams {
            start: None, end: None, page: None, query_type: None, start_cycle: None, end_cycle: None, sender: "".to_string(), sign: None
        }).await?;

        let (total_receipts, total_cycles, total_original_txs) = match total_data {
            Some(d) => (d.total_receipts, d.total_cycles, d.total_original_txs),
            None => {
                tracing::error!("Failed to fetch total data");
                return Ok(());
            }
        };

        tracing::info!("Sync Status: Receipts: {}/{}, Cycles: {}/{}, OriginalTxs: {}/{}",
            last_receipt_count, total_receipts,
            last_cycle_count, total_cycles,
            last_original_tx_count, total_original_txs);

        if last_receipt_count >= total_receipts && last_cycle_count >= total_cycles && last_original_tx_count >= total_original_txs {
            tracing::info!("Data is up to date.");
            return Ok(());
        }

        // 3. Sync Genesis (Cycles 0-5)
        if last_cycle_count == 0 {
             self.sync_genesis().await?;
             // Update counts
             last_receipt_count = self.storage.get_receipt_count().await?;
             last_original_tx_count = self.storage.get_original_tx_data_count().await?;
             last_cycle_count = self.storage.get_cycle_count().await?;
        }

        // 4. Download missing data
        self.download_txs_and_cycles(
            total_receipts, last_receipt_count,
            total_original_txs, last_original_tx_count,
            total_cycles, last_cycle_count
        ).await?;

        Ok(())
    }

    async fn sync_genesis(&self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("Syncing Genesis Data...");
        // Accounts
        let start_cycle = 0;
        let end_cycle = 5;
        let mut page = 1;
        loop {
            let res = self.query::<AccountResponse>(DataType::Account, QueryParams {
                start: None, end: None, page: Some(page), query_type: None,
                start_cycle: Some(start_cycle), end_cycle: Some(end_cycle), sender: "".to_string(), sign: None
            }).await?;

            if let Some(data) = res {
                if data.accounts.is_empty() { break; }
                let acc_len = data.accounts.len();
                for acc in data.accounts {
                    self.storage.insert_account(&acc).await?;
                }
                 if acc_len < self.config.request_limits.max_accounts_per_request as usize {
                    break;
                }
                page += 1;
            } else {
                break;
            }
        }

        // Transactions
        page = 1;
        loop {
             let res = self.query::<TransactionResponse>(DataType::Transaction, QueryParams {
                start: None, end: None, page: Some(page), query_type: None,
                start_cycle: Some(start_cycle), end_cycle: Some(end_cycle), sender: "".to_string(), sign: None
            }).await?;

             if let Some(data) = res {
                if data.transactions.is_empty() { break; }
                let tx_len = data.transactions.len();
                for tx in data.transactions {
                    self.storage.insert_transaction(&tx).await?;
                }
                 if tx_len < self.config.request_limits.max_transactions_per_request as usize {
                    break;
                }
                page += 1;
            } else {
                break;
            }
        }

        Ok(())
    }

    async fn download_txs_and_cycles(&self,
        total_receipts: i64, from_receipt: i64,
        total_original_txs: i64, from_original_tx: i64,
        total_cycles: i64, from_cycle: i64
    ) -> Result<(), Box<dyn std::error::Error>> {

        // Receipts
        let mut start = from_receipt;
        while start < total_receipts {
            let end = start + self.config.request_limits.max_receipts_per_request;
            tracing::info!("Downloading receipts {} - {}", start, end);
            let res = self.query::<ReceiptResponse>(DataType::Receipt, QueryParams {
                start: Some(start), end: Some(end), page: None, query_type: None, start_cycle: None, end_cycle: None, sender: "".to_string(), sign: None
            }).await?;

            if let Some(data) = res {
                if data.receipts.is_empty() { break; }
                // Use process_receipts to handle side-effects (accounts/transactions update)
                self.storage.process_receipts(&data.receipts).await?;
                start = end + 1;
            } else {
                break;
            }
        }

        // Original Txs
        let mut start = from_original_tx;
        while start < total_original_txs {
            let end = start + self.config.request_limits.max_original_txs_per_request;
            tracing::info!("Downloading original txs {} - {}", start, end);
            let res = self.query::<OriginalTxResponse>(DataType::OriginalTx, QueryParams {
                start: Some(start), end: Some(end), page: None, query_type: None, start_cycle: None, end_cycle: None, sender: "".to_string(), sign: None
            }).await?;
             if let Some(data) = res {
                if data.original_txs.is_empty() { break; }
                for tx in data.original_txs {
                    self.storage.insert_original_tx(&tx).await?;
                }
                start = end + 1;
            } else {
                break;
            }
        }

        // Cycles
        let mut start = from_cycle;
        while start < total_cycles {
            let end = start + self.config.request_limits.max_cycles_per_request;
            tracing::info!("Downloading cycles {} - {}", start, end);
             let res = self.query::<CycleResponse>(DataType::Cycle, QueryParams {
                start: Some(start), end: Some(end), page: None, query_type: None, start_cycle: None, end_cycle: None, sender: "".to_string(), sign: None
            }).await?;
             if let Some(data) = res {
                if data.cycle_info.is_empty() { break; }
                for c in data.cycle_info {
                    self.storage.insert_cycle(&c).await?;
                }
                start = end + 1;
            } else {
                break;
            }
        }

        Ok(())
    }

    async fn connect_to_distributor(&self) {
        let mut connected = false;

        while !connected {
             // 1. Prepare Connection URL
            let collector_info = serde_json::json!({
                "subscriptionType": "FIREHOSE",
                "timestamp": chrono::Utc::now().timestamp_millis(),
            });

            let payload = serde_json::json!({
                "collectorInfo": collector_info,
                "sender": self.config.collector_info.public_key
            });

            // Sign the payload (as string)
            let msg_string = serde_json::to_string(&payload).unwrap();
            let kp = self.crypto.get_key_pair_using_sk(&HexStringOrBuffer::Hex(self.config.collector_info.secret_key.clone()));
            let sig = self.crypto.sign(HexStringOrBuffer::Hex(msg_string.clone()), &kp.secret_key).unwrap();
            let sig_hex = hex::encode(sig);

            // Construct Signed Object
            let signed_payload = serde_json::json!({
                "collectorInfo": collector_info,
                "sender": self.config.collector_info.public_key,
                "sign": sig_hex
            });

            let query_string = serde_json::to_string(&signed_payload).unwrap();
            let encoded_query = urlencoding::encode(&query_string);

            let url_str = format!("ws://{}:{}/?data={}",
                self.config.distributor_info.ip,
                self.config.distributor_info.port,
                encoded_query
            );

            tracing::info!("Connecting to Distributor at {}", url_str);

            match Url::parse(&url_str) {
                Ok(url) => {
                     match connect_async(url.as_str()).await {
                        Ok((ws_stream, _)) => {
                            tracing::info!("Connected to Distributor");
                            connected = true;

                            let (_write, mut read) = ws_stream.split();

                            while let Some(message) = read.next().await {
                                match message {
                                    Ok(Message::Text(text)) => {
                                        // Process Data
                                        if let Err(e) = self.process_firehose_data(&text).await {
                                            tracing::error!("Error processing firehose data: {}", e);
                                        }
                                    },
                                    Ok(Message::Close(_)) => {
                                        tracing::warn!("Distributor closed connection");
                                        connected = false;
                                        break;
                                    },
                                    Err(e) => {
                                        tracing::error!("WebSocket Error: {}", e);
                                        connected = false;
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                        },
                        Err(e) => {
                             tracing::error!("Failed to connect: {}", e);
                             sleep(Duration::from_millis(self.config.distributor_reconnect_interval as u64)).await;
                        }
                    }
                },
                Err(e) => {
                     tracing::error!("Invalid URL: {}", e);
                     return;
                }
            }
        }
    }

    async fn process_firehose_data(&self, data: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Data comes as { cycle, receipt, originalTx, ... }
        let val: Value = serde_json::from_str(data)?;

        // Check for specific fields and insert
        if let Some(cycle) = val.get("cycle") {
             let c: Cycle = serde_json::from_value(cycle.clone())?;
             self.storage.insert_cycle(&c).await?;
             tracing::debug!("Saved Cycle {}", c.counter);
        }

        if let Some(receipt) = val.get("receipt") {
             let r: Receipt = serde_json::from_value(receipt.clone())?;
             // Use process_receipts for side effects
             self.storage.process_receipts(&[r.clone()]).await?;
             tracing::debug!("Saved Receipt {}", r.receipt_id);
        }

        if let Some(original_tx) = val.get("originalTx") {
             let tx: OriginalTxData = serde_json::from_value(original_tx.clone())?;
             self.storage.insert_original_tx(&tx).await?;
             tracing::debug!("Saved OriginalTx {}", tx.tx_id);
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let config = load_config();
    let storage = Storage::init(&config).await.expect("Failed to init storage");

    let collector = Arc::new(Collector::new(config, storage));

    // Initial Sync
    if let Err(e) = collector.sync_data().await {
        tracing::error!("Initial Sync Failed: {}", e);
    }

    // Connect to Distributor (Long running)
    collector.connect_to_distributor().await;
}

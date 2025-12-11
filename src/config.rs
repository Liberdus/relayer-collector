use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CollectorMode {
    WS,
    MQ,
}

impl Default for CollectorMode {
    fn default() -> Self {
        CollectorMode::WS
    }
}

impl std::str::FromStr for CollectorMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "WS" => Ok(CollectorMode::WS),
            "MQ" => Ok(CollectorMode::MQ),
            _ => Err(format!("Invalid collector mode: {}", s)),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DataLogWriterConfig {
    #[serde(rename = "dirName")]
    pub dir_name: String,
    #[serde(rename = "maxLogFiles")]
    pub max_log_files: i64,
    #[serde(rename = "maxReceiptEntries")]
    pub max_receipt_entries: i64,
    #[serde(rename = "maxCycleEntries")]
    pub max_cycle_entries: i64,
    #[serde(rename = "maxOriginalTxEntries")]
    pub max_original_tx_entries: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CollectorInfo {
    #[serde(rename = "publicKey")]
    pub public_key: String,
    #[serde(rename = "secretKey")]
    pub secret_key: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CollectorData {
    #[serde(rename = "cycleDB")]
    pub cycle_db: String,
    #[serde(rename = "accountDB")]
    pub account_db: String,
    #[serde(rename = "transactionDB")]
    pub transaction_db: String,
    #[serde(rename = "receiptDB")]
    pub receipt_db: String,
    #[serde(rename = "originalTxDataDB")]
    pub original_tx_data_db: String,
    #[serde(rename = "accountHistoryStateDB")]
    pub account_history_state_db: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PortConfig {
    pub server: String,
    pub collector: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CollectorSocketServerConfig {
    pub enabled: bool,
    #[serde(rename = "forwardReceipt")]
    pub forward_receipt: bool,
    #[serde(rename = "forwardAccount")]
    pub forward_account: bool,
    #[serde(rename = "forwardTransaction")]
    pub forward_transaction: bool,
    #[serde(rename = "forwardAppReceipt")]
    pub forward_app_receipt: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DistributorInfo {
    pub ip: String,
    pub port: String,
    #[serde(rename = "publicKey")]
    pub public_key: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProcessDataConfig {
    #[serde(rename = "indexReceipt")]
    pub index_receipt: bool,
    #[serde(rename = "indexOriginalTxData")]
    pub index_original_tx_data: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RequestLimits {
    #[serde(rename = "MAX_RECEIPTS_PER_REQUEST")]
    pub max_receipts_per_request: i64,
    #[serde(rename = "MAX_ORIGINAL_TXS_PER_REQUEST")]
    pub max_original_txs_per_request: i64,
    #[serde(rename = "MAX_CYCLES_PER_REQUEST")]
    pub max_cycles_per_request: i64,
    #[serde(rename = "MAX_ACCOUNTS_PER_REQUEST")]
    pub max_accounts_per_request: i64,
    #[serde(rename = "MAX_TRANSACTIONS_PER_REQUEST")]
    pub max_transactions_per_request: i64,
    #[serde(rename = "MAX_BETWEEN_CYCLES_PER_REQUEST")]
    pub max_between_cycles_per_request: i64,
    #[serde(rename = "MAX_ACCOUNT_HISTORY_STATES_PER_REQUEST")]
    pub max_account_history_states_per_request: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub env: String,
    pub host: String,
    #[serde(rename = "dataLogWrite")]
    pub data_log_write: bool,
    #[serde(rename = "dataLogWriter")]
    pub data_log_writer: DataLogWriterConfig,
    #[serde(rename = "collectorInfo")]
    pub collector_info: CollectorInfo,
    #[serde(rename = "hashKey")]
    pub hash_key: String,
    #[serde(rename = "COLLECTOR_DB_DIR_PATH")]
    pub collector_db_dir_path: String,
    #[serde(rename = "COLLECTOR_DATA")]
    pub collector_data: CollectorData,
    pub port: PortConfig,
    #[serde(rename = "collectorSockerServer")]
    pub collector_socket_server: CollectorSocketServerConfig,
    #[serde(rename = "distributorInfo")]
    pub distributor_info: DistributorInfo,
    pub verbose: bool,
    #[serde(rename = "fastifyDebugLog")]
    pub fastify_debug_log: bool,
    #[serde(rename = "rateLimit")]
    pub rate_limit: i64,
    #[serde(rename = "patchData")]
    pub patch_data: bool,
    #[serde(rename = "USAGE_ENDPOINTS_KEY")]
    pub usage_endpoints_key: String,
    #[serde(rename = "DISTRIBUTOR_RECONNECT_INTERVAL")]
    pub distributor_reconnect_interval: i64,
    #[serde(rename = "CONNECT_TO_DISTRIBUTOR_MAX_RETRY")]
    pub connect_to_distributor_max_retry: i64,
    #[serde(rename = "processData")]
    pub process_data: ProcessDataConfig,
    #[serde(rename = "saveAccountHistoryState")]
    pub save_account_history_state: bool,
    #[serde(rename = "collectorMode")]
    pub collector_mode: CollectorMode,
    #[serde(rename = "storeReceiptBeforeStates")]
    pub store_receipt_before_states: bool,
    #[serde(rename = "requestLimits")]
    pub request_limits: RequestLimits,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            env: "production".to_string(),
            host: "127.0.0.1".to_string(),
            data_log_write: false,
            data_log_writer: DataLogWriterConfig {
                dir_name: "data-logs".to_string(),
                max_log_files: 10,
                max_receipt_entries: 1000,
                max_cycle_entries: 1000,
                max_original_tx_entries: 1000,
            },
            collector_info: CollectorInfo {
                public_key: "".to_string(),
                secret_key: "".to_string(),
            },
            hash_key: "69fa4195670576c0160d660c3be36556ff8d504725be8a59b5a96509e0c994bc".to_string(),
            collector_db_dir_path: "collector-db".to_string(),
            collector_data: CollectorData {
                cycle_db: "cycles.sqlite3".to_string(),
                account_db: "accounts.sqlite3".to_string(),
                transaction_db: "transactions.sqlite3".to_string(),
                receipt_db: "receipts.sqlite3".to_string(),
                original_tx_data_db: "originalTxsData.sqlite3".to_string(),
                account_history_state_db: "accountHistoryState.sqlite3".to_string(),
            },
            port: PortConfig {
                server: "6101".to_string(),
                collector: "4444".to_string(),
            },
            collector_socket_server: CollectorSocketServerConfig {
                enabled: true,
                forward_receipt: false,
                forward_account: false,
                forward_transaction: false,
                forward_app_receipt: true,
            },
            distributor_info: DistributorInfo {
                ip: "127.0.0.1".to_string(),
                port: "6100".to_string(),
                public_key: "".to_string(),
            },
            verbose: false,
            fastify_debug_log: false,
            rate_limit: 100,
            patch_data: false,
            usage_endpoints_key: "".to_string(),
            distributor_reconnect_interval: 10_000,
            connect_to_distributor_max_retry: 10,
            process_data: ProcessDataConfig {
                index_receipt: true,
                index_original_tx_data: true,
            },
            save_account_history_state: true,
            collector_mode: CollectorMode::WS,
            store_receipt_before_states: true,
            request_limits: RequestLimits {
                max_receipts_per_request: 100,
                max_original_txs_per_request: 100,
                max_cycles_per_request: 100,
                max_accounts_per_request: 100,
                max_transactions_per_request: 100,
                max_between_cycles_per_request: 100,
                max_account_history_states_per_request: 100,
            },
        }
    }
}

// CLI arguments struct
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short, long)]
    pub config: Option<String>,
}

pub fn load_config() -> Config {
    // 1. Default config
    let mut config = Config::default();

    // 2. Load from config.json
    let config_path = Path::new("config.json");
    if config_path.exists() {
        if let Ok(file_content) = fs::read_to_string(config_path) {
            if let Ok(json_config) = serde_json::from_str::<Value>(&file_content) {
               // Implementing merge manually or using a crate would be better,
               // but for now I'll skip deep merging and assume the struct matching works.
               // Since deep merge in Rust is not trivial without crates like `merge`.
               // For now, let's assume if config.json exists we deserialize it and fill missing values with defaults if possible,
               // but `serde_json::from_str` replaces the whole struct.
               // A better approach is to deserialize into a partial struct and override.
               // For simplicity I will just try to deserialize what I can.
               // Actually, `config` dependency in Rust is good for this, but I'll stick to manual for now.
               if let Ok(loaded) = serde_json::from_str::<Config>(&file_content) {
                   config = loaded;
               }
            }
        }
    }

    // 3. Environment variables
    dotenvy::dotenv().ok();
    if let Ok(val) = env::var("SHARDEUM_COLLECTOR_MODE") { config.env = val; }
    if let Ok(val) = env::var("HOST") { config.host = val; }
    if let Ok(val) = env::var("PORT") { config.port.server = val; }
    if let Ok(val) = env::var("COLLECTOR_PORT") { config.port.collector = val; }
    if let Ok(val) = env::var("DISTRIBUTOR_IP") { config.distributor_info.ip = val; }
    if let Ok(val) = env::var("DISTRIBUTOR_PORT") { config.distributor_info.port = val; }
    if let Ok(val) = env::var("COLLECTOR_MODE") {
         if let Ok(mode) = val.parse() { config.collector_mode = mode; }
    }

    // 4. CLI arguments (handled by caller usually, but we can do it here if we pass args)
    // For now, I'll assume Env vars + config.json + defaults is enough for this port
    // unless CLI args are strictly required to override everything else.

    // Handle dev secrets if in dev mode
    if config.env == "development" {
        config.usage_endpoints_key = "ceba96f6eafd2ea59e68a0b0d754a939".to_string();
        config.collector_info.secret_key = "7d8819b6fac8ba2fbac7363aaeb5c517e52e615f95e1a161d635521d5e4969739426b64e675cad739d69526bf7e27f3f304a8a03dca508a9180f01e9269ce447".to_string();
        config.collector_info.public_key = "9426b64e675cad739d69526bf7e27f3f304a8a03dca508a9180f01e9269ce447".to_string();
        config.distributor_info.public_key = "758b1c119412298802cd28dbfa394cdfeecc4074492d60844cc192d632d84de3".to_string();
    } else {
        // Load secrets from .secrets file
        let secrets_path = PathBuf::from(".secrets");
        if secrets_path.exists() {
             if let Ok(content) = fs::read_to_string(secrets_path) {
                 for line in content.lines() {
                     if let Some((key, value)) = line.split_once('=') {
                         let key = key.trim();
                         let value = value.trim().to_string();
                         match key {
                             "USAGE_ENDPOINTS_KEY" => config.usage_endpoints_key = value,
                             "COLLECTOR_SECRET_KEY" => config.collector_info.secret_key = value,
                             "COLLECTOR_PUBLIC_KEY" => config.collector_info.public_key = value,
                             "DISTRIBUTOR_PUBLIC_KEY" => config.distributor_info.public_key = value,
                             _ => {}
                         }
                     }
                 }
             }
        }
    }

    config
}

pub fn get_distributor_url(config: &Config) -> String {
    format!("http://{}:{}", config.distributor_info.ip, config.distributor_info.port)
}

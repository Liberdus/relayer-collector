use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize};
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use collector_rust::{
    config::{load_config, Config},
    storage::Storage,
    types::{
        AccountResponse, ReceiptResponse, TransactionResponse,
        OriginalTxResponse,
    },
};

struct AppState {
    storage: Storage,
    #[allow(dead_code)]
    config: Config,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = load_config();
    let storage = Storage::init(&config).await.expect("Failed to init storage");
    let port = config.port.server.parse::<u16>().unwrap_or(6101);

    let state = Arc::new(AppState { storage, config });

    let app = Router::new()
        .route("/api/cycleinfo", get(get_cycle_info))
        .route("/api/account", get(get_account))
        .route("/api/transaction", get(get_transaction))
        .route("/api/receipt", get(get_receipt))
        .route("/api/originalTx", get(get_original_tx))
        .route("/totalData", get(get_total_data))
        .route("/is-alive", get(health_check))
        .route("/is-healthy", get(health_check))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Server listening on {}", addr);
    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

#[derive(Deserialize)]
struct CycleInfoQuery {
    count: Option<i64>,
    #[serde(rename = "cycleNumber")]
    cycle_number: Option<i64>,
    start: Option<i64>,
    end: Option<i64>,
    marker: Option<String>,
}

async fn get_cycle_info(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CycleInfoQuery>,
) -> impl IntoResponse {
    let res = if let Some(count) = query.count {
        let cycles = state
            .storage
            .get_latest_cycle_records(count)
            .await
            .unwrap_or_default();
        serde_json::json!({ "success": true, "cycles": cycles })
    } else if let Some(cycle_number) = query.cycle_number {
        let cycle = state
            .storage
            .get_cycle_by_counter(cycle_number)
            .await
            .unwrap_or(None);
        let cycles = if let Some(c) = cycle { vec![c] } else { vec![] };
        serde_json::json!({ "success": true, "cycles": cycles })
    } else if let Some(start) = query.start {
        if let Some(end) = query.end {
             let cycles = state
                .storage
                .get_cycles_between(start, end)
                .await
                .unwrap_or_default();
             serde_json::json!({ "success": true, "cycles": cycles })
        } else {
             return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "success": false, "error": "Missing end parameter" }))).into_response();
        }
    } else if let Some(marker) = query.marker {
         let cycle = state
            .storage
            .get_cycle_by_marker(&marker)
            .await
            .unwrap_or(None);
        let cycles = if let Some(c) = cycle { vec![c] } else { vec![] };
        serde_json::json!({ "success": true, "cycles": cycles })
    } else {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "success": false, "error": "No valid query parameter provided" }))).into_response();
    };

    Json(res).into_response()
}

#[derive(Deserialize)]
struct AccountQuery {
    count: Option<i64>,
    page: Option<i64>,
    #[serde(rename = "accountId")]
    account_id: Option<String>,
    #[serde(rename = "accountSearchType")]
    #[allow(dead_code)]
    account_search_type: Option<String>, // Handling enum manually might be easier if it comes as string
}

async fn get_account(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AccountQuery>,
) -> impl IntoResponse {
    if let Some(count) = query.count {
        let accounts = state.storage.get_accounts(0, count).await.unwrap_or_default();
        let total_accounts = state.storage.get_account_count().await.unwrap_or(0);
        return Json(AccountResponse {
            success: true,
            accounts: Some(accounts),
            total_pages: None,
            total_accounts: Some(total_accounts),
        }).into_response();
    } else if let Some(account_id) = query.account_id {
        let account = state.storage.get_account_by_id(&account_id).await.unwrap_or(None);
        let accounts = if let Some(a) = account { vec![a] } else { vec![] };
        return Json(AccountResponse {
            success: true,
            accounts: Some(accounts),
            total_pages: None,
            total_accounts: None,
        }).into_response();
    } else if let Some(page) = query.page {
        let items_per_page = 10;
        let offset = (page - 1) * items_per_page;
        let accounts = state.storage.get_accounts(offset, items_per_page).await.unwrap_or_default();
        let total_accounts = state.storage.get_account_count().await.unwrap_or(0);
        let total_pages = (total_accounts as f64 / items_per_page as f64).ceil() as i64;
        return Json(AccountResponse {
             success: true,
            accounts: Some(accounts),
            total_pages: Some(total_pages),
            total_accounts: Some(total_accounts),
        }).into_response();
    }

     (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "success": false, "error": "Invalid parameters" }))).into_response()
}

#[derive(Deserialize)]
struct TransactionQuery {
    count: Option<i64>,
    page: Option<i64>,
    #[serde(rename = "txId")]
    tx_id: Option<String>,
    #[serde(rename = "appReceiptId")]
    app_receipt_id: Option<String>,
}

async fn get_transaction(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TransactionQuery>,
) -> impl IntoResponse {
    if let Some(count) = query.count {
        let txs = state.storage.get_transactions(0, count).await.unwrap_or_default();
        let total = state.storage.get_transaction_count().await.unwrap_or(0);
         return Json(TransactionResponse {
            success: true,
            transactions: Some(txs),
            total_pages: None,
            total_transactions: Some(total),
        }).into_response();
    } else if let Some(page) = query.page {
         let items_per_page = 10;
        let offset = (page - 1) * items_per_page;
        let txs = state.storage.get_transactions(offset, items_per_page).await.unwrap_or_default();
        let total = state.storage.get_transaction_count().await.unwrap_or(0);
        let total_pages = (total as f64 / items_per_page as f64).ceil() as i64;
        return Json(TransactionResponse {
            success: true,
            transactions: Some(txs),
            total_pages: Some(total_pages),
            total_transactions: Some(total),
        }).into_response();
    } else if let Some(tx_id) = query.tx_id {
         let tx = state.storage.get_transaction_by_id(&tx_id).await.unwrap_or(None);
         let txs = if let Some(t) = tx { vec![t] } else { vec![] };
         return Json(TransactionResponse {
            success: true,
            transactions: Some(txs),
            total_pages: None,
            total_transactions: None,
        }).into_response();
    } else if let Some(app_receipt_id) = query.app_receipt_id {
          let tx = state.storage.get_transaction_by_app_receipt_id(&app_receipt_id).await.unwrap_or(None);
         let txs = if let Some(t) = tx { vec![t] } else { vec![] };
         return Json(TransactionResponse {
            success: true,
            transactions: Some(txs),
            total_pages: None,
            total_transactions: None,
        }).into_response();
    }

     (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "success": false, "error": "Invalid params" }))).into_response()
}

#[derive(Deserialize)]
struct ReceiptQuery {
    count: Option<i64>,
    page: Option<i64>,
    #[serde(rename = "receiptId")]
    receipt_id: Option<String>,
}

async fn get_receipt(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ReceiptQuery>,
) -> impl IntoResponse {
    if let Some(count) = query.count {
        let receipts = state.storage.get_receipts(0, count).await.unwrap_or_default();
        let total = state.storage.get_receipt_count().await.unwrap_or(0);
         return Json(ReceiptResponse {
            success: true,
            receipts: Some(receipts),
            total_pages: None,
            total_receipts: Some(total),
        }).into_response();
    } else if let Some(receipt_id) = query.receipt_id {
         let receipt = state.storage.get_receipt_by_id(&receipt_id).await.unwrap_or(None);
         let receipts = if let Some(r) = receipt { vec![r] } else { vec![] };
         return Json(ReceiptResponse {
            success: true,
            receipts: Some(receipts),
            total_pages: None,
            total_receipts: None,
        }).into_response();
    } else if let Some(page) = query.page {
         let items_per_page = 10;
        let offset = (page - 1) * items_per_page;
        let receipts = state.storage.get_receipts(offset, items_per_page).await.unwrap_or_default();
        let total = state.storage.get_receipt_count().await.unwrap_or(0);
        let total_pages = (total as f64 / items_per_page as f64).ceil() as i64;
        return Json(ReceiptResponse {
            success: true,
            receipts: Some(receipts),
            total_pages: Some(total_pages),
            total_receipts: Some(total),
        }).into_response();
    }
     (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "success": false, "error": "Invalid parameters" }))).into_response()
}

#[derive(Deserialize)]
struct OriginalTxQuery {
    count: Option<i64>,
    page: Option<i64>,
}

async fn get_original_tx(
    State(state): State<Arc<AppState>>,
    Query(query): Query<OriginalTxQuery>,
) -> impl IntoResponse {
     if let Some(count) = query.count {
        let txs = state.storage.get_original_txs(0, count).await.unwrap_or_default();
        let total = state.storage.get_original_tx_data_count().await.unwrap_or(0);
         return Json(OriginalTxResponse {
            success: true,
            original_txs: Some(txs),
            total_pages: None,
            total_original_txs: Some(total),
        }).into_response();
    } else if let Some(page) = query.page {
         let items_per_page = 10;
        let offset = (page - 1) * items_per_page;
        let txs = state.storage.get_original_txs(offset, items_per_page).await.unwrap_or_default();
        let total = state.storage.get_original_tx_data_count().await.unwrap_or(0);
        let total_pages = (total as f64 / items_per_page as f64).ceil() as i64;
        return Json(OriginalTxResponse {
            success: true,
            original_txs: Some(txs),
            total_pages: Some(total_pages),
            total_original_txs: Some(total),
        }).into_response();
    }
     (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "success": false, "error": "Invalid parameters" }))).into_response()
}

async fn get_total_data(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let total_cycles = state.storage.get_cycle_count().await.unwrap_or(0);
    let total_receipts = state.storage.get_receipt_count().await.unwrap_or(0);
    let total_original_txs = state.storage.get_original_tx_data_count().await.unwrap_or(0);
    let total_accounts = state.storage.get_account_count().await.unwrap_or(0);
    let total_transactions = state.storage.get_transaction_count().await.unwrap_or(0);

    Json(serde_json::json!({
        "totalCycles": total_cycles,
        "totalReceipts": total_receipts,
        "totalOriginalTxs": total_original_txs,
        "totalAccounts": total_accounts,
        "totalTransactions": total_transactions
    }))
}

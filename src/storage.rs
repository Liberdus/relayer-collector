use sqlx::sqlite::{SqlitePool, SqlitePoolOptions, SqliteRow};
use sqlx::Row;
use std::path::Path;
use tokio::fs;

use crate::config::Config;
use crate::types::{
    Account, AccountType, Cycle, OriginalTxData, Receipt, Transaction,
    ArchiverReceipt, ArchiverReceiptTx, TransactionType, AccountHistoryState
};

pub struct Storage {
    pub account_db: SqlitePool,
    pub cycle_db: SqlitePool,
    pub transaction_db: SqlitePool,
    pub receipt_db: SqlitePool,
    pub original_tx_data_db: SqlitePool,
    pub account_history_state_db: SqlitePool,
    pub config: Config, // Store config to access flags
}

impl Storage {
    pub async fn init(config: &Config) -> Result<Self, sqlx::Error> {
        // Create directory if it doesn't exist
        if !Path::new(&config.collector_db_dir_path).exists() {
            fs::create_dir_all(&config.collector_db_dir_path)
                .await
                .expect("Failed to create DB directory");
        }

        let account_db = Self::connect_db(
            &config.collector_db_dir_path,
            &config.collector_data.account_db,
        )
        .await?;
        let cycle_db = Self::connect_db(
            &config.collector_db_dir_path,
            &config.collector_data.cycle_db,
        )
        .await?;
        let transaction_db = Self::connect_db(
            &config.collector_db_dir_path,
            &config.collector_data.transaction_db,
        )
        .await?;
        let receipt_db = Self::connect_db(
            &config.collector_db_dir_path,
            &config.collector_data.receipt_db,
        )
        .await?;
        let original_tx_data_db = Self::connect_db(
            &config.collector_db_dir_path,
            &config.collector_data.original_tx_data_db,
        )
        .await?;
        let account_history_state_db = Self::connect_db(
            &config.collector_db_dir_path,
            &config.collector_data.account_history_state_db,
        )
        .await?;

        // Initialize Tables
        // Cycle
        sqlx::query(
            "CREATE TABLE if not exists `cycles` (`cycleMarker` TEXT NOT NULL UNIQUE PRIMARY KEY, `counter` NUMBER NOT NULL, `cycleRecord` JSON NOT NULL)"
        ).execute(&cycle_db).await?;
        sqlx::query(
            "CREATE INDEX if not exists `cycles_idx` ON `cycles` (`counter` DESC)"
        ).execute(&cycle_db).await?;

        // Account
        sqlx::query(
            "CREATE TABLE if not exists `accounts` (`accountId` TEXT NOT NULL UNIQUE PRIMARY KEY, `data` JSON NOT NULL, `timestamp` BIGINT NOT NULL, `hash` TEXT NOT NULL, `cycleNumber` NUMBER NOT NULL, `isGlobal` BOOLEAN NOT NULL, `accountType` TEXT NOT NULL)"
        ).execute(&account_db).await?;
        sqlx::query(
            "CREATE INDEX if not exists `accounts_idx` ON `accounts` (`cycleNumber` DESC, `timestamp` DESC)"
        ).execute(&account_db).await?;

        // Transaction
        sqlx::query(
            "CREATE TABLE if not exists `transactions` (`txId` TEXT NOT NULL UNIQUE PRIMARY KEY, `appReceiptId` TEXT, `timestamp` BIGINT NOT NULL, `cycleNumber` NUMBER NOT NULL, `data` JSON NOT NULL, `originalTxData` JSON NOT NULL, `transactionType` TEXT, `txFrom` TEXT, `txTo` TEXT, `nominee` TEXT)"
        ).execute(&transaction_db).await?;
        sqlx::query("CREATE INDEX if not exists `transactions_timestamp` ON `transactions` (`timestamp` DESC)").execute(&transaction_db).await?;
        sqlx::query("CREATE INDEX if not exists `transactions_cycle` ON `transactions` (`cycleNumber` DESC)").execute(&transaction_db).await?;

        // Receipt
        sqlx::query(
            "CREATE TABLE if not exists `receipts` (`receiptId` TEXT NOT NULL UNIQUE PRIMARY KEY, `tx` JSON NOT NULL, `cycle` NUMBER NOT NULL, `applyTimestamp` BIGINT NOT NULL, `timestamp` BIGINT NOT NULL, `signedReceipt` JSON NOT NULL, `afterStates` JSON, `beforeStates` JSON, `appReceiptData` JSON, `executionShardKey` TEXT NOT NULL, `globalModification` BOOLEAN NOT NULL)"
        ).execute(&receipt_db).await?;
        sqlx::query("CREATE INDEX if not exists `receipts_timestamp` ON `receipts` (`timestamp` DESC)").execute(&receipt_db).await?;
        sqlx::query("CREATE INDEX if not exists `receipts_cycle` ON `receipts` (`cycle` DESC)").execute(&receipt_db).await?;

        // OriginalTxData
        sqlx::query(
            "CREATE TABLE if not exists `originalTxsData` (`txId` TEXT NOT NULL, `timestamp` BIGINT NOT NULL, `cycle` NUMBER NOT NULL, `originalTxData` JSON NOT NULL, `transactionType` TEXT, `txFrom` TEXT, `txTo` TEXT, PRIMARY KEY (`txId`, `timestamp`))"
        ).execute(&original_tx_data_db).await?;
        sqlx::query("CREATE INDEX if not exists `originalTxsData_timestamp` ON `originalTxsData` (`timestamp` DESC)").execute(&original_tx_data_db).await?;

        // AccountHistoryState
        sqlx::query(
            "CREATE TABLE if not exists `accountHistoryState` (`accountId` TEXT NOT NULL, `beforeStateHash` TEXT NOT NULL, `afterStateHash` TEXT NOT NULL, `timestamp` BIGINT NOT NULL, `receiptId` TEXT NOT NULL, PRIMARY KEY (`accountId`, `timestamp`))"
        ).execute(&account_history_state_db).await?;

        Ok(Storage {
            account_db,
            cycle_db,
            transaction_db,
            receipt_db,
            original_tx_data_db,
            account_history_state_db,
            config: config.clone(),
        })
    }

    async fn connect_db(dir: &str, filename: &str) -> Result<SqlitePool, sqlx::Error> {
        let db_path = format!("{}/{}", dir, filename);
        let url = format!("sqlite://{}?mode=rwc", db_path);
        SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
    }

    pub async fn close(&self) {
        self.account_db.close().await;
        self.cycle_db.close().await;
        self.transaction_db.close().await;
        self.receipt_db.close().await;
        self.original_tx_data_db.close().await;
        self.account_history_state_db.close().await;
    }

    // --- Data Processing High Level ---

    pub async fn process_receipts(&self, receipts: &[Receipt]) -> Result<(), sqlx::Error> {
        // Just loop for now, bulk insert could be optimized
        for r in receipts {
            // 1. Insert Receipt
            self.insert_receipt(r).await?;

            // 2. Process Accounts from afterStates
            if self.config.process_data.index_receipt {
                 if let Some(after_states) = &r.archiver_receipt.after_states {
                     for account in after_states {
                         // Check timestamp
                         let existing = self.get_account_by_id(&account.account_id).await?;
                         let should_update = match existing {
                             Some(e) => account.timestamp > e.timestamp,
                             None => true,
                         };

                         if should_update {
                             self.insert_account(account).await?;
                         }
                     }
                 }
            }

            // 3. Process Transaction
            let tx_obj = self.extract_transaction_from_receipt(r);
            let existing_tx = self.get_transaction_by_id(&tx_obj.tx_id).await?;
            let should_update_tx = match existing_tx {
                Some(e) => tx_obj.timestamp > e.timestamp,
                None => true,
            };
            if should_update_tx {
                self.insert_transaction(&tx_obj).await?;
            }

            // 4. Process AccountHistoryState
            if self.config.save_account_history_state {
                 if !r.archiver_receipt.global_modification {
                      let proposal = &r.archiver_receipt.signed_receipt.proposal;
                      if !proposal.account_ids.is_empty() {
                           for (i, account_id) in proposal.account_ids.iter().enumerate() {
                               let before = proposal.before_state_hashes.get(i).map(|s| s.clone()).unwrap_or_default();
                               let after = proposal.after_state_hashes.get(i).map(|s| s.clone()).unwrap_or_default();
                               let ahs = AccountHistoryState {
                                   account_id: account_id.clone(),
                                   before_state_hash: before,
                                   after_state_hash: after,
                                   timestamp: r.timestamp,
                                   receipt_id: r.receipt_id.clone(),
                               };
                               self.insert_account_history_state(&ahs).await?;
                           }
                      }
                 }
            }
        }
        Ok(())
    }

    fn extract_transaction_from_receipt(&self, receipt: &Receipt) -> Transaction {
        let r = &receipt.archiver_receipt;
        let mut tx = Transaction {
            tx_id: r.tx.tx_id.clone(),
            app_receipt_id: None,
            timestamp: r.tx.timestamp,
            cycle_number: r.cycle,
            data: serde_json::Value::Object(serde_json::Map::new()),
            original_tx_data: r.tx.original_tx_data.clone(),
            transaction_type: None,
            tx_from: None,
            tx_to: None,
        };

        // Try to check app_receipt_data first (similar to TS logic)
        // If app_receipt_data is not null/empty
        if !r.app_receipt_data.is_null() && r.app_receipt_data.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
             let app_data = &r.app_receipt_data;
             // Extract fields from app_data
             if let Some(t) = app_data.get("type").and_then(|v| v.as_str()) {
                 tx.transaction_type = serde_json::from_value(serde_json::Value::String(t.to_string())).ok();
             }
             tx.tx_from = app_data.get("from").and_then(|v| v.as_str()).map(|s| s.to_string());
             tx.tx_to = app_data.get("to").and_then(|v| v.as_str()).map(|s| s.to_string());
             tx.app_receipt_id = app_data.get("appReceiptId").and_then(|v| v.as_str()).map(|s| s.to_string());
             tx.data = app_data.clone();
        } else {
            // Fallback to original tx data
            // r.tx.original_tx_data.tx
            if let Some(otx) = r.tx.original_tx_data.get("tx") {
                 if let Some(t) = otx.get("type").and_then(|v| v.as_str()) {
                     tx.transaction_type = serde_json::from_value(serde_json::Value::String(t.to_string())).ok();
                 }
                 tx.tx_from = otx.get("from").and_then(|v| v.as_str()).map(|s| s.to_string());
                 tx.tx_to = otx.get("to").and_then(|v| v.as_str()).map(|s| s.to_string());
                 tx.app_receipt_id = otx.get("appReceiptId").and_then(|v| v.as_str()).map(|s| s.to_string());

                 // Special cases for some types (simplified logic from TS)
                 if let Some(tt) = &tx.transaction_type {
                     match tt {
                         TransactionType::create => {
                             tx.tx_to = tx.tx_from.clone();
                         }
                         TransactionType::register => {
                              tx.tx_to = otx.get("aliasHash").and_then(|v| v.as_str()).map(|s| s.to_string());
                         }
                         TransactionType::deposit_stake | TransactionType::withdraw_stake => {
                             tx.tx_from = otx.get("nominator").and_then(|v| v.as_str()).map(|s| s.to_string());
                             tx.tx_to = otx.get("nominee").and_then(|v| v.as_str()).map(|s| s.to_string());
                         }
                         // Add other cases as needed
                         _ => {}
                     }
                 }
            }
        }

        tx
    }

    pub async fn insert_account_history_state(&self, ahs: &AccountHistoryState) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT OR REPLACE INTO accountHistoryState (accountId, beforeStateHash, afterStateHash, timestamp, receiptId) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&ahs.account_id)
        .bind(&ahs.before_state_hash)
        .bind(&ahs.after_state_hash)
        .bind(ahs.timestamp)
        .bind(&ahs.receipt_id)
        .execute(&self.account_history_state_db)
        .await?;
        Ok(())
    }


    // Queries

    // --- Cycles ---
    pub async fn get_cycle_count(&self) -> Result<i64, sqlx::Error> {
        let result: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM cycles")
            .fetch_one(&self.cycle_db)
            .await?;
        Ok(result.0)
    }

    pub async fn get_latest_cycle_records(&self, count: i64) -> Result<Vec<Cycle>, sqlx::Error> {
        sqlx::query_as::<_, Cycle>(
            r#"SELECT cycleMarker as "cycle_marker", counter, cycleRecord as "cycle_record" FROM cycles ORDER BY counter DESC LIMIT ?"#
        )
        .bind(count)
        .fetch_all(&self.cycle_db)
        .await
    }

    pub async fn get_cycle_by_counter(&self, counter: i64) -> Result<Option<Cycle>, sqlx::Error> {
         sqlx::query_as::<_, Cycle>(
            r#"SELECT cycleMarker as "cycle_marker", counter, cycleRecord as "cycle_record" FROM cycles WHERE counter = ?"#
        )
        .bind(counter)
        .fetch_optional(&self.cycle_db)
        .await
    }

    pub async fn get_cycle_by_marker(&self, marker: &str) -> Result<Option<Cycle>, sqlx::Error> {
        sqlx::query_as::<_, Cycle>(
            r#"SELECT cycleMarker as "cycle_marker", counter, cycleRecord as "cycle_record" FROM cycles WHERE cycleMarker = ?"#
        )
        .bind(marker)
        .fetch_optional(&self.cycle_db)
        .await
    }

    pub async fn get_cycles_between(&self, from: i64, to: i64) -> Result<Vec<Cycle>, sqlx::Error> {
         sqlx::query_as::<_, Cycle>(
            r#"SELECT cycleMarker as "cycle_marker", counter, cycleRecord as "cycle_record" FROM cycles WHERE counter BETWEEN ? AND ? ORDER BY counter ASC"#
        )
        .bind(from)
        .bind(to)
        .fetch_all(&self.cycle_db)
        .await
    }

    pub async fn insert_cycle(&self, cycle: &Cycle) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT OR REPLACE INTO cycles (cycleMarker, counter, cycleRecord) VALUES (?, ?, ?)"
        )
        .bind(&cycle.cycle_marker)
        .bind(cycle.counter)
        .bind(&cycle.cycle_record)
        .execute(&self.cycle_db)
        .await?;
        Ok(())
    }

    // --- Accounts ---
    pub async fn get_account_count(&self) -> Result<i64, sqlx::Error> {
         let result: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts")
            .fetch_one(&self.account_db)
            .await?;
        Ok(result.0)
    }

    pub async fn get_accounts(&self, offset: i64, limit: i64) -> Result<Vec<Account>, sqlx::Error> {
        let rows = sqlx::query(
            r#"SELECT accountId, data, timestamp, hash, cycleNumber, isGlobal, accountType FROM accounts ORDER BY cycleNumber DESC, timestamp DESC LIMIT ? OFFSET ?"#
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.account_db)
        .await?;

        let accounts = rows.into_iter().map(|r| {
             let account_type: String = r.get("accountType");
             Account {
                account_id: r.get("accountId"),
                data: r.get("data"),
                timestamp: r.get("timestamp"),
                hash: r.get("hash"),
                cycle_number: r.get("cycleNumber"),
                is_global: r.get("isGlobal"),
                account_type: if account_type == "UserAccount" { Some(AccountType::UserAccount) } else { None },
            }
        }).collect();
        Ok(accounts)
    }

    pub async fn get_account_by_id(&self, account_id: &str) -> Result<Option<Account>, sqlx::Error> {
        let row = sqlx::query(
            r#"SELECT accountId, data, timestamp, hash, cycleNumber, isGlobal, accountType FROM accounts WHERE accountId = ?"#
        )
        .bind(account_id)
        .fetch_optional(&self.account_db)
        .await?;

        Ok(row.map(|r| {
            let account_type: String = r.get("accountType");
            Account {
                account_id: r.get("accountId"),
                data: r.get("data"),
                timestamp: r.get("timestamp"),
                hash: r.get("hash"),
                cycle_number: r.get("cycleNumber"),
                is_global: r.get("isGlobal"),
                account_type: if account_type == "UserAccount" { Some(AccountType::UserAccount) } else { None },
            }
        }))
    }

    pub async fn insert_account(&self, account: &Account) -> Result<(), sqlx::Error> {
        let account_type_str = account.account_type.as_ref().map(|t| match t {
            AccountType::UserAccount => "UserAccount",
        }).unwrap_or("UserAccount");

        sqlx::query(
            "INSERT OR REPLACE INTO accounts (accountId, data, timestamp, hash, cycleNumber, isGlobal, accountType) VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&account.account_id)
        .bind(&account.data)
        .bind(account.timestamp)
        .bind(&account.hash)
        .bind(account.cycle_number)
        .bind(account.is_global)
        .bind(account_type_str)
        .execute(&self.account_db)
        .await?;
        Ok(())
    }


    // --- Receipts ---
    pub async fn get_receipt_count(&self) -> Result<i64, sqlx::Error> {
         let result: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM receipts")
            .fetch_one(&self.receipt_db)
            .await?;
        Ok(result.0)
    }

     pub async fn get_receipts(&self, offset: i64, limit: i64) -> Result<Vec<Receipt>, sqlx::Error> {
        let rows = sqlx::query(
            r#"SELECT receiptId, tx, cycle, applyTimestamp, timestamp, signedReceipt, afterStates, beforeStates, appReceiptData, executionShardKey, globalModification FROM receipts ORDER BY cycle DESC, timestamp DESC LIMIT ? OFFSET ?"#
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.receipt_db)
        .await?;

        let receipts = rows.into_iter().map(|r| {
             let tx_val: serde_json::Value = r.get("tx");
             let signed_receipt_val: serde_json::Value = r.get("signedReceipt");
             let after_states_val: Option<serde_json::Value> = r.get("afterStates");
             let before_states_val: Option<serde_json::Value> = r.get("beforeStates");
             let app_receipt_data_val: Option<serde_json::Value> = r.get("appReceiptData");

             let archiver_receipt = ArchiverReceipt {
                 tx: serde_json::from_value(tx_val).unwrap_or_else(|_| ArchiverReceiptTx { original_tx_data: serde_json::Value::Null, tx_id: "".to_string(), timestamp: 0 }),
                 cycle: r.get("cycle"),
                 signed_receipt: serde_json::from_value(signed_receipt_val).unwrap(),
                 after_states: after_states_val.and_then(|v| serde_json::from_value(v).ok()),
                 before_states: before_states_val.and_then(|v| serde_json::from_value(v).ok()),
                 app_receipt_data: app_receipt_data_val.unwrap_or(serde_json::Value::Null),
                 execution_shard_key: r.get("executionShardKey"),
                 global_modification: r.get("globalModification"),
             };
             Receipt {
                 archiver_receipt,
                 receipt_id: r.get("receiptId"),
                 timestamp: r.get("timestamp"),
                 apply_timestamp: r.get("applyTimestamp"),
             }
        }).collect();

        Ok(receipts)
    }

    pub async fn get_receipt_by_id(&self, receipt_id: &str) -> Result<Option<Receipt>, sqlx::Error> {
         let r = sqlx::query(
            r#"SELECT receiptId, tx, cycle, applyTimestamp, timestamp, signedReceipt, afterStates, beforeStates, appReceiptData, executionShardKey, globalModification FROM receipts WHERE receiptId = ?"#
        )
        .bind(receipt_id)
        .fetch_optional(&self.receipt_db)
        .await?;

        if let Some(r) = r {
             let tx_val: serde_json::Value = r.get("tx");
             let signed_receipt_val: serde_json::Value = r.get("signedReceipt");
             let after_states_val: Option<serde_json::Value> = r.get("afterStates");
             let before_states_val: Option<serde_json::Value> = r.get("beforeStates");
             let app_receipt_data_val: Option<serde_json::Value> = r.get("appReceiptData");

             let archiver_receipt = ArchiverReceipt {
                 tx: serde_json::from_value(tx_val).unwrap(),
                 cycle: r.get("cycle"),
                 signed_receipt: serde_json::from_value(signed_receipt_val).unwrap(),
                 after_states: after_states_val.and_then(|v| serde_json::from_value(v).ok()),
                 before_states: before_states_val.and_then(|v| serde_json::from_value(v).ok()),
                 app_receipt_data: app_receipt_data_val.unwrap_or(serde_json::Value::Null),
                 execution_shard_key: r.get("executionShardKey"),
                 global_modification: r.get("globalModification"),
             };
             return Ok(Some(Receipt {
                 archiver_receipt,
                 receipt_id: r.get("receiptId"),
                 timestamp: r.get("timestamp"),
                 apply_timestamp: r.get("applyTimestamp"),
             }));
        }
        Ok(None)
    }

    pub async fn insert_receipt(&self, receipt: &Receipt) -> Result<(), sqlx::Error> {
        let tx_json = serde_json::to_value(&receipt.archiver_receipt.tx).unwrap();
        let signed_receipt_json = serde_json::to_value(&receipt.archiver_receipt.signed_receipt).unwrap();
        let after_states_json = serde_json::to_value(&receipt.archiver_receipt.after_states).ok();
        let before_states_json = serde_json::to_value(&receipt.archiver_receipt.before_states).ok();

        sqlx::query(
            "INSERT OR REPLACE INTO receipts (receiptId, tx, cycle, applyTimestamp, timestamp, signedReceipt, afterStates, beforeStates, appReceiptData, executionShardKey, globalModification) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&receipt.receipt_id)
        .bind(tx_json)
        .bind(receipt.archiver_receipt.cycle)
        .bind(receipt.apply_timestamp)
        .bind(receipt.timestamp)
        .bind(signed_receipt_json)
        .bind(after_states_json)
        .bind(before_states_json)
        .bind(&receipt.archiver_receipt.app_receipt_data)
        .bind(&receipt.archiver_receipt.execution_shard_key)
        .bind(receipt.archiver_receipt.global_modification)
        .execute(&self.receipt_db)
        .await?;
        Ok(())
    }

    // --- OriginalTxData ---
    pub async fn get_original_tx_data_count(&self) -> Result<i64, sqlx::Error> {
         let result: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM originalTxsData")
            .fetch_one(&self.original_tx_data_db)
            .await?;
        Ok(result.0)
    }

    pub async fn get_original_txs(&self, offset: i64, limit: i64) -> Result<Vec<OriginalTxData>, sqlx::Error> {
        let rows = sqlx::query(
            r#"SELECT txId, timestamp, cycle, originalTxData, transactionType, txFrom, txTo FROM originalTxsData ORDER BY cycle DESC, timestamp DESC LIMIT ? OFFSET ?"#
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.original_tx_data_db)
        .await?;

        let txs = rows.into_iter().map(|r| {
             let tx_type_str: Option<String> = r.get("transactionType");
             let transaction_type = tx_type_str.and_then(|s| {
                 serde_json::from_value(serde_json::Value::String(s)).ok()
             });

            OriginalTxData {
                tx_id: r.get("txId"),
                timestamp: r.get("timestamp"),
                cycle: r.get("cycle"),
                original_tx_data: r.get("originalTxData"),
                transaction_type,
                tx_from: r.get("txFrom"),
                tx_to: r.get("txTo"),
            }
        }).collect();
        Ok(txs)
    }

    pub async fn insert_original_tx(&self, tx: &OriginalTxData) -> Result<(), sqlx::Error> {
        // transactionType needs to be handled
        let type_str = tx.transaction_type.as_ref().map(|t| format!("{:?}", t));

        sqlx::query(
            "INSERT OR REPLACE INTO originalTxsData (txId, timestamp, cycle, originalTxData, transactionType, txFrom, txTo) VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&tx.tx_id)
        .bind(tx.timestamp)
        .bind(tx.cycle)
        .bind(&tx.original_tx_data)
        .bind(type_str)
        .bind(&tx.tx_from)
        .bind(&tx.tx_to)
        .execute(&self.original_tx_data_db)
        .await?;
        Ok(())
    }

    // --- Transactions ---
    pub async fn get_transaction_count(&self) -> Result<i64, sqlx::Error> {
         let result: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions")
            .fetch_one(&self.transaction_db)
            .await?;
        Ok(result.0)
    }

    pub async fn get_transactions(&self, offset: i64, limit: i64) -> Result<Vec<Transaction>, sqlx::Error> {
         let rows = sqlx::query(
            r#"SELECT txId, appReceiptId, timestamp, cycleNumber, data, originalTxData, transactionType, txFrom, txTo FROM transactions ORDER BY cycleNumber DESC, timestamp DESC LIMIT ? OFFSET ?"#
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.transaction_db)
        .await?;

        let txs = rows.into_iter().map(|r| {
             let tx_type_str: Option<String> = r.get("transactionType");
             let transaction_type = tx_type_str.and_then(|s| {
                 serde_json::from_value(serde_json::Value::String(s)).ok()
             });

             Transaction {
                tx_id: r.get("txId"),
                app_receipt_id: r.get("appReceiptId"),
                timestamp: r.get("timestamp"),
                cycle_number: r.get("cycleNumber"),
                data: r.get("data"),
                original_tx_data: r.get("originalTxData"),
                transaction_type,
                tx_from: r.get("txFrom"),
                tx_to: r.get("txTo"),
            }
        }).collect();
        Ok(txs)
    }

    pub async fn get_transaction_by_id(&self, tx_id: &str) -> Result<Option<Transaction>, sqlx::Error> {
         let r = sqlx::query(
            r#"SELECT txId, appReceiptId, timestamp, cycleNumber, data, originalTxData, transactionType, txFrom, txTo FROM transactions WHERE txId = ?"#
        )
        .bind(tx_id)
        .fetch_optional(&self.transaction_db)
        .await?;

        if let Some(r) = r {
             let tx_type_str: Option<String> = r.get("transactionType");
             let transaction_type = tx_type_str.and_then(|s| {
                 serde_json::from_value(serde_json::Value::String(s)).ok()
             });

             return Ok(Some(Transaction {
                tx_id: r.get("txId"),
                app_receipt_id: r.get("appReceiptId"),
                timestamp: r.get("timestamp"),
                cycle_number: r.get("cycleNumber"),
                data: r.get("data"),
                original_tx_data: r.get("originalTxData"),
                transaction_type,
                tx_from: r.get("txFrom"),
                tx_to: r.get("txTo"),
            }));
        }
        Ok(None)
    }

    pub async fn get_transaction_by_app_receipt_id(&self, app_receipt_id: &str) -> Result<Option<Transaction>, sqlx::Error> {
         let r = sqlx::query(
            r#"SELECT txId, appReceiptId, timestamp, cycleNumber, data, originalTxData, transactionType, txFrom, txTo FROM transactions WHERE appReceiptId = ?"#
        )
        .bind(app_receipt_id)
        .fetch_optional(&self.transaction_db)
        .await?;

        if let Some(r) = r {
             let tx_type_str: Option<String> = r.get("transactionType");
             let transaction_type = tx_type_str.and_then(|s| {
                 serde_json::from_value(serde_json::Value::String(s)).ok()
             });

             return Ok(Some(Transaction {
                tx_id: r.get("txId"),
                app_receipt_id: r.get("appReceiptId"),
                timestamp: r.get("timestamp"),
                cycle_number: r.get("cycleNumber"),
                data: r.get("data"),
                original_tx_data: r.get("originalTxData"),
                transaction_type,
                tx_from: r.get("txFrom"),
                tx_to: r.get("txTo"),
            }));
        }
        Ok(None)
    }

    pub async fn insert_transaction(&self, tx: &Transaction) -> Result<(), sqlx::Error> {
         let type_str = tx.transaction_type.as_ref().map(|t| format!("{:?}", t));

         sqlx::query(
            "INSERT OR REPLACE INTO transactions (txId, appReceiptId, timestamp, cycleNumber, data, originalTxData, transactionType, txFrom, txTo, nominee) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&tx.tx_id)
        .bind(&tx.app_receipt_id)
        .bind(tx.timestamp)
        .bind(tx.cycle_number)
        .bind(&tx.data)
        .bind(&tx.original_tx_data)
        .bind(type_str)
        .bind(&tx.tx_from)
        .bind(&tx.tx_to)
        .bind("") // Nominee?
        .execute(&self.transaction_db)
        .await?;
        Ok(())
    }

}

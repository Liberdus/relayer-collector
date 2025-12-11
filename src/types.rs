use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use serde_json::Value;
use serde_json::Value as JsonValue;

// Mocks for external types
pub type Signature = String; // In TS code it uses @shardus/crypto-utils Signature, which is usually a string

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AccountType {
    UserAccount,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AccountSearchParams {
    All,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum AccountSearchType {
    AccountType(AccountType),
    AccountSearchParams(AccountSearchParams),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Account {
    #[serde(rename = "accountId")]
    pub account_id: String,
    pub data: Value,
    pub timestamp: i64,
    pub hash: String,
    #[serde(rename = "cycleNumber")]
    pub cycle_number: i64,
    #[serde(rename = "isGlobal")]
    pub is_global: bool,
    #[serde(rename = "accountType")]
    pub account_type: Option<AccountType>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[derive(FromRow)]
pub struct Cycle {
    #[serde(rename = "cycleMarker")]
    pub cycle_marker: String,
    pub counter: i64,
    #[serde(rename = "cycleRecord")]
    pub cycle_record: Value, // Using Value as the structure of CycleData is complex and external
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TxDataType {
    RECEIPT,
    #[serde(rename = "ORIGINAL_TX_DATA")]
    OriginalTxData,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DistributorSocketCloseCodes {
    DuplicateConnectionCode = 1000,
    SubscriberExpirationCode,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OriginalTxData {
    #[serde(rename = "txId")]
    pub tx_id: String,
    pub timestamp: i64,
    pub cycle: i64,
    #[serde(rename = "originalTxData")]
    pub original_tx_data: Value,
    #[serde(rename = "transactionType")]
    pub transaction_type: Option<TransactionType>,
    #[serde(rename = "txFrom")]
    pub tx_from: String,
    #[serde(rename = "txTo")]
    pub tx_to: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Proposal {
    pub applied: bool,
    #[serde(rename = "cant_preApply")]
    pub cant_pre_apply: bool,
    #[serde(rename = "accountIDs")]
    pub account_ids: Vec<String>,
    #[serde(rename = "beforeStateHashes")]
    pub before_state_hashes: Vec<String>,
    #[serde(rename = "afterStateHashes")]
    pub after_state_hashes: Vec<String>,
    #[serde(rename = "appReceiptDataHash")]
    pub app_receipt_data_hash: String,
    pub txid: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Vote {
    #[serde(rename = "proposalHash")]
    pub proposal_hash: String,
    pub sign: Option<Signature>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SignedReceipt {
    pub proposal: Proposal,
    #[serde(rename = "proposalHash")]
    pub proposal_hash: String,
    #[serde(rename = "signaturePack")]
    pub signature_pack: Vec<Signature>,
    #[serde(rename = "voteOffsets")]
    pub vote_offsets: Vec<i64>,
    pub sign: Option<Signature>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ArchiverReceipt {
    pub tx: ArchiverReceiptTx,
    pub cycle: i64,
    #[serde(rename = "signedReceipt")]
    pub signed_receipt: SignedReceipt,
    #[serde(rename = "afterStates")]
    pub after_states: Option<Vec<Account>>,
    #[serde(rename = "beforeStates")]
    pub before_states: Option<Vec<Account>>,
    #[serde(rename = "appReceiptData")]
    pub app_receipt_data: Value,
    #[serde(rename = "executionShardKey")]
    pub execution_shard_key: String,
    #[serde(rename = "globalModification")]
    pub global_modification: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ArchiverReceiptTx {
    #[serde(rename = "originalTxData")]
    pub original_tx_data: Value,
    #[serde(rename = "txId")]
    pub tx_id: String,
    pub timestamp: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Receipt {
    #[serde(flatten)]
    pub archiver_receipt: ArchiverReceipt,
    #[serde(rename = "receiptId")]
    pub receipt_id: String,
    pub timestamp: i64,
    #[serde(rename = "applyTimestamp")]
    pub apply_timestamp: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AccountHistoryState {
    #[serde(rename = "accountId")]
    pub account_id: String,
    #[serde(rename = "beforeStateHash")]
    pub before_state_hash: String,
    #[serde(rename = "afterStateHash")]
    pub after_state_hash: String,
    pub timestamp: i64,
    #[serde(rename = "receiptId")]
    pub receipt_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppliedVote {
    pub txid: String,
    pub transaction_result: bool,
    pub account_id: Vec<String>,
    pub account_state_hash_after: Vec<String>,
    pub account_state_hash_before: Vec<String>,
    pub cant_apply: bool,
    pub node_id: String,
    pub sign: Signature,
    pub app_data_hash: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConfirmOrChallengeMessage {
    pub message: String,
    #[serde(rename = "nodeId")]
    pub node_id: String,
    #[serde(rename = "appliedVote")]
    pub applied_vote: AppliedVote,
    pub sign: Signature,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppliedReceipt2 {
    pub txid: String,
    pub result: bool,
    #[serde(rename = "appliedVote")]
    pub applied_vote: AppliedVote,
    #[serde(rename = "confirmOrChallenge")]
    pub confirm_or_challenge: ConfirmOrChallengeMessage,
    pub signatures: Vec<Signature>,
    pub app_data_hash: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ReceiptResponse {
    pub success: bool,
    pub receipts: Option<Vec<Receipt>>, // changed from unknown to Vec<Receipt>
    #[serde(rename = "totalPages")]
    pub total_pages: Option<i64>,
    #[serde(rename = "totalReceipts")]
    pub total_receipts: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OriginalTxResponse {
    pub success: bool,
    #[serde(rename = "originalTxs")]
    pub original_txs: Option<Vec<OriginalTxData>>,
    #[serde(rename = "totalPages")]
    pub total_pages: Option<i64>,
    #[serde(rename = "totalOriginalTxs")]
    pub total_original_txs: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransactionResponse {
    pub success: bool,
    pub transactions: Option<Vec<Transaction>>,
    #[serde(rename = "totalPages")]
    pub total_pages: Option<i64>,
    #[serde(rename = "totalTransactions")]
    pub total_transactions: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AccountResponse {
    pub success: bool,
    pub accounts: Option<Vec<Account>>,
    #[serde(rename = "totalPages")]
    pub total_pages: Option<i64>,
    #[serde(rename = "totalAccounts")]
    pub total_accounts: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Transaction {
    #[serde(rename = "txId")]
    pub tx_id: String,
    #[serde(rename = "appReceiptId")]
    pub app_receipt_id: Option<String>,
    pub timestamp: i64,
    #[serde(rename = "cycleNumber")]
    pub cycle_number: i64,
    pub data: Value,
    #[serde(rename = "originalTxData")]
    pub original_tx_data: Value,
    #[serde(rename = "transactionType")]
    pub transaction_type: Option<TransactionType>,
    #[serde(rename = "txFrom")]
    pub tx_from: Option<String>,
    #[serde(rename = "txTo")]
    pub tx_to: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_camel_case_types)]
pub enum TransactionType {
    init_network,
    network_windows,
    snapshot,
    email,
    gossip_email_hash,
    verify,
    register,
    create,
    transfer,
    distribute,
    message,
    toll,
    friend,
    remove_friend,
    stake,
    remove_stake,
    remove_stake_request,
    node_reward,
    snapshot_claim,
    issue,
    proposal,
    vote,
    tally,
    apply_tally,
    parameters,
    apply_parameters,
    dev_issue,
    dev_proposal,
    dev_vote,
    dev_tally,
    apply_dev_tally,
    dev_parameters,
    apply_dev_parameters,
    developer_payment,
    apply_developer_payment,
    change_config,
    apply_change_config,
    change_network_param,
    apply_change_network_param,
    deposit_stake,
    withdraw_stake,
    set_cert_time,
    query_certificate,
    init_reward,
    claim_reward,
    apply_penalty,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TransactionSearchParams {
    All,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum TransactionSearchType {
    TransactionType(TransactionType),
    TransactionSearchParams(TransactionSearchParams),
}

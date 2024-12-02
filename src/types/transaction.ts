export interface Transaction {
  txId: string
  appReceiptId?: string // Dapp receipt id (eg. txhash for the EVM receipt in shardeum)
  timestamp: number
  cycleNumber: number
  data: any & { txId?: string; appReceiptId?: string }
  originalTxData: unknown
  transactionType?: TransactionType
  txFrom?: string
  txTo?: string
}

export enum TransactionType {
  Transfer = 'transfer',
  Message = 'message',
}

export enum TransactionSearchType {
  All = 'all',
  Transfer = 'transfer',
  Message = 'message',
  // e.g AllExceptInternalTx = 1 for all except InternalTx
}

export type TxStatus = 'Pending' | 'Expired'

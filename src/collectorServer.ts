import { WebSocketServer, WebSocket } from 'ws'
import { config as CONFIG } from './config'
import { Account, Receipt, Transaction, AppReceipt, TransactionType } from './types'
import { Utils as StringUtils } from '@shardus/types'
import * as crypto from '@shardus/crypto-utils'

export const ReceiptDataWsEvent = '/data/receipt'
export const AccountDataWsEvent = '/data/account'
export const TransactionDataWsEvent = '/data/transaction'
export const AppReceiptDataWsEvent = '/data/appReceipt'

const forwardReceipt = false
const forwardAccount = false
const forwardTransaction = true
const forwardAppReceipt = true

const subscribers = new Map<string, WebSocket>()

export const setupCollectorSocketServer = (): void => {
  const wss = new WebSocketServer({ port: Number(CONFIG.port.collector) })

  wss.on('connection', (ws: WebSocket, req) => {
    const id = crypto.randomBytes(16).toString()
    console.log(`New notification server registered ${id}`)
    subscribers.set(id, ws)

    ws.on('close', () => {
      console.log(`subscriber ${id} disconnected`)
      subscribers.delete(id)
    })

    ws.on('error', (err) => {
      console.log(`notification ${id} error: ${err}. Disconnecting...`)
      subscribers.delete(id)
      ws.close()
    })
  })

  console.log(`AccountUpdate sender listening on port ${CONFIG.port.collector}`)
}

export const forwardData = (receipt: Receipt): void => {
  const { cycle, appReceiptData, tx } = receipt

  if (forwardReceipt) {
    sendToSubscribers(ReceiptDataWsEvent, receipt)
  }

  if (forwardAccount) {
    // Extract account from receipt afterStates and send to subscribers
  }

  if (forwardTransaction) {
    // Extract transaction from receipt and send to subscribers
    const txObj = {
      txId: tx.txId,
      cycleNumber: cycle,
      timestamp: tx.timestamp,
      originalTxData: tx.originalTxData || {},
    } as Transaction

    if (appReceiptData) {
      txObj.transactionType = appReceiptData.type as TransactionType // be sure to update with the correct field with the transaction type defined in the dapp
      txObj.txFrom = appReceiptData.from // be sure to update with the correct field of the tx sender
      txObj.txTo = appReceiptData.to // be sure to update with the correct field of the tx recipient
      txObj.data = appReceiptData
      txObj.appReceiptId = appReceiptData.appReceiptId
    }
    sendToSubscribers(TransactionDataWsEvent, txObj)
  }

  if (forwardAppReceipt) {
    // Extract appReceipt from receipt and send to subscribers
    const appReceiptData = receipt.appReceiptData
    sendToSubscribers(AppReceiptDataWsEvent, appReceiptData)
  }
}

export const sendToSubscribers = async (
  type: string,
  data: Account | Receipt | Transaction | AppReceipt
): Promise<void> => {
  if (
    type !== AccountDataWsEvent &&
    type !== ReceiptDataWsEvent &&
    type !== TransactionDataWsEvent &&
    type !== AppReceiptDataWsEvent
  ) {
    console.log('Unknown data type, skip sending to subscribers', type)
    return
  }
  if (subscribers.size === 0) {
    console.log('No notification service connected, skip sending to subscribers')
    return
  }

  const message = JSON.stringify({
    event: type,
    data: StringUtils.safeStringify(data),
  })

  for (const [id, ws] of subscribers.entries()) {
    if (ws.readyState === WebSocket.OPEN) {
      ws.send(message)
    } else {
      subscribers.delete(id)
    }
  }

  if (CONFIG.verbose) console.log(`Sent data to ${subscribers.size} subscribers`)
}

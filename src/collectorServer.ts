import { WebSocketServer, WebSocket } from 'ws'
import { config as CONFIG } from './config'
import { Account, Receipt, Transaction, AppReceipt, TransactionType } from './types'
import { Utils as StringUtils } from '@shardus/types'
import * as crypto from '@shardus/crypto-utils'

export enum SubscriptionType {
  RECEIPT = '/data/receipt',
  ACCOUNT = '/data/account',
  TRANSACTION = '/data/transaction',
  APP_RECEIPT = '/data/appReceipt',
}

export interface SubscriberInfo {
  ws: WebSocket
  subscriptions: SubscriptionType[]
}

export const ReceiptDataWsEvent = '/data/receipt'
export const AccountDataWsEvent = '/data/account'
export const TransactionDataWsEvent = '/data/transaction'
export const AppReceiptDataWsEvent = '/data/appReceipt'

const forwardReceipt = false
const forwardAccount = false
const forwardTransaction = true
const forwardAppReceipt = false

type SubscriberId = string

const subscribers = new Map<SubscriberId, SubscriberInfo>()
const subscribersByType = new Map<SubscriptionType, Set<SubscriberId>>()

// Initialize subscription type maps
Object.values(SubscriptionType).forEach((type) => {
  subscribersByType.set(type, new Set<string>())
})

/**
 * Client Connection Guide:
 * -----------------------
 * Connect with subscription types as URL parameters:
 *
 * Example client code:
 * ```javascript
 * public connect(subscriptionTypes: string[] = ['/data/transaction']): void {
 *   const params = new URLSearchParams()
 *   params.set('subscriptions', JSON.stringify(subscriptionTypes))
 *   const url = `ws://${this.config.host}:${this.config.port}?${params.toString()}`
 *
 *   this.ws = new WebSocket(url)
 *   // ... rest of connection handling
 * }
 * ```
 *
 * Available subscription types:
 * - '/data/receipt' - Raw receipt data
 * - '/data/account' - Account state changes
 * - '/data/transaction' - Transaction data
 * - '/data/appReceipt' - Application receipt data
 *
 * Note: Subscriptions are set at connection time and cannot be changed dynamically.
 * To change subscriptions, disconnect and reconnect with new parameters.
 */
export const setupCollectorSocketServer = (): void => {
  const wss = new WebSocketServer({ port: Number(CONFIG.port.collector) })

  wss.on('connection', (ws: WebSocket, req) => {
    const id = crypto.randomBytes(16).toString()
    console.log(`New notification server registered ${id}`)

    // Parse subscription types from URL parameters
    const url = new URL(req.url || '', `http://${req.headers.host}`)
    const subscriptionsParam = url.searchParams.get('subscriptions')

    let subscriptions: SubscriptionType[] = []

    if (subscriptionsParam) {
      try {
        const parsedSubscriptions = JSON.parse(subscriptionsParam)
        subscriptions = parsedSubscriptions.filter((type: string) =>
          Object.values(SubscriptionType).includes(type as SubscriptionType)
        )
      } catch (error) {
        console.log(`Invalid subscriptions parameter from ${id}:`, error)
      }
    }

    // Default to all subscription types
    if (subscriptions.length === 0) {
      subscriptions = Object.values(SubscriptionType)
      console.log(`${id} using default subscriptions (all types)`)
    } else {
      console.log(`${id} subscribed to:`, subscriptions)
    }

    const subscriberInfo: SubscriberInfo = {
      ws,
      subscriptions,
    }

    subscribers.set(id, subscriberInfo)

    // Add to type-specific maps for fast lookup
    subscriptions.forEach((type) => {
      subscribersByType.get(type)?.add(id)
    })

    ws.on('close', () => {
      console.log(`subscriber ${id} disconnected`)
      removeSubscriber(id)
    })

    ws.on('error', (err) => {
      console.log(`notification ${id} error: ${err}. Disconnecting...`)
      removeSubscriber(id)
      ws.close()
    })
  })

  console.log(`AccountUpdate sender listening on port ${CONFIG.port.collector}`)
}

const removeSubscriber = (id: string): void => {
  const subscriberInfo = subscribers.get(id)
  if (subscriberInfo) {
    // Remove from type-specific maps
    subscriberInfo.subscriptions.forEach((type) => {
      subscribersByType.get(type)?.delete(id)
    })
    // Remove from main subscribers map
    subscribers.delete(id)
  }
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

  // Get subscribers for this specific event type (fast lookup)
  const eventType = type as SubscriptionType
  const typeSubscriberIds = subscribersByType.get(eventType)

  if (!typeSubscriberIds || typeSubscriberIds.size === 0) {
    if (CONFIG.verbose) console.log(`No subscribers for event type: ${eventType}`)
    return
  }

  for (const id of typeSubscriberIds) {
    const subscriberInfo = subscribers.get(id)
    if (subscriberInfo && subscriberInfo.ws.readyState === WebSocket.OPEN) {
      subscriberInfo.ws.send(message)
    } else {
      removeSubscriber(id)
    }
  }

  if (CONFIG.verbose) console.log(`Sent data to ${typeSubscriberIds.size} subscribers for type: ${eventType}`)
}

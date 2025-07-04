import { WebSocketServer, WebSocket } from 'ws'
import { config as CONFIG } from './config'
import { Account, Receipt, Transaction, AppReceipt } from './types'
import { Utils as StringUtils } from '@shardus/types'
import * as crypto from '@shardus/crypto-utils'

export const ReceiptDataWsEvent = '/data/receipt'
export const AccountDataWsEvent = '/data/account'
export const TransactionDataWsEvent = '/data/transaction'
export const AppReceiptDataWsEvent = '/data/appReceipt'

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

export const forwardData = async (
  type: string,
  data: Account | Receipt | Transaction | AppReceipt
): Promise<void> => {
  if (
    type !== AccountDataWsEvent &&
    type !== ReceiptDataWsEvent &&
    type !== TransactionDataWsEvent &&
    type !== AppReceiptDataWsEvent
  ) {
    console.log('Unknown data type, skipping forwarding to subscribers', type)
    return
  }
  if (subscribers.size === 0) {
    console.log('No notification service connected, skipping forwardingdata')
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

  if (CONFIG.verbose) console.log(`Forwarded data to ${subscribers.size} notification servers`)
}

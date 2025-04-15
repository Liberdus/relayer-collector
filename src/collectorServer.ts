import { WebSocketServer, WebSocket } from 'ws'
import { config as CONFIG } from './config'
import { Account, Receipt } from './types'
import { Utils as StringUtils } from '@shardus/types'
import * as crypto from '@shardus/crypto-utils'

export const ReceiptDataWsEvent = '/data/receipt'

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

export const forwardLatestAccount = async (data: Account): Promise<void> => {
  if (subscribers.size === 0) {
    console.log('No notification service connected, skipping sending receipt data')
    return
  }

  const message = JSON.stringify({
    event: ReceiptDataWsEvent,
    data: StringUtils.safeStringify(data),
  })

  for (const [id, ws] of subscribers.entries()) {
    if (ws.readyState === WebSocket.OPEN) {
      ws.send(message)
    } else {
      subscribers.delete(id)
    }
  }

  if (CONFIG.verbose)
    console.log(`Forwarded receipt data to ${subscribers.size} notification servers`)
}

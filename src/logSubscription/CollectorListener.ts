import fs from 'fs'
import * as socketClient from 'socket.io-client'
import { config } from '../config'
import { IndexedLogs, extractLogsFromReceipts } from './CollectorDataParser'
import { getLogSocketClient, logSubscriptionMap } from './SocketManager'
import { ArchivedCycles, ArchivedReceipts } from './types'

export const setupCollectorListener = async (): Promise<void> => {
  const socket = socketClient.connect(`http://${config.host}:${config.port.collector}`, {
    reconnection: true,
    reconnectionAttempts: 10,
  })

  // Register default socket event handlers
  socket.on('connect', () => console.log('Connected to distributor sender'))
  socket.on('disconnect', () => console.log('Disconnected from distributor sender'))
  socket.on('error', (err) => console.log(`Error from distributor sender: ${err}`))

  // Register custom socket event handlers
  socket.on('/data/cycle', cycleDataHandler)
  socket.on('/data/receipt', receiptDataHandler)
}

const cycleDataHandler = async (data: ArchivedCycles): Promise<void> => {
  /*prettier-ignore*/ console.log('Received archived cycle data, valid? ', data.archivedCycles && data.archivedCycles[0] && data.archivedCycles[0].cycleRecord && data.archivedCycles[0].cycleRecord.counter)
  console.log(`Cycle data: ${JSON.stringify(data, null, 2)}`)
}

const receiptDataHandler = async (data: ArchivedReceipts): Promise<void> => {
  /*prettier-ignore*/ console.log('Received receipt data from archiver.')

  const logs = extractLogsFromReceipts(data)
  console.log(`Number of logs found in receipts: ${logs.length}`)

  if (logs.length == 0) {
    return
  }

  const indexedLogs = new IndexedLogs(logs)

  for (const [subscriptionId, subscription] of logSubscriptionMap.entries()) {
    const filteredLogs = indexedLogs.filter(subscription.filterOptions)
    console.log(`Number of logs found for subscription ${subscriptionId}: ${filteredLogs.length}`)
    if (filteredLogs.length > 0) {
      const socket = getLogSocketClient(subscription.socketId)
      if (socket) {
        console.log(`Sending ${filteredLogs.length} logs to socket ${subscription.socketId}`)
        await socket.socket.send(
          JSON.stringify({ method: 'log_found', subscription_id: subscriptionId, logs: filteredLogs })
        )
      }
    }
  }
}

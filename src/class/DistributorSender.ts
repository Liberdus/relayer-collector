import { Server, Socket } from 'socket.io'
import { Data } from './validateData'
import { config as CONFIG } from '../config'

// constants
const ConnectionEvent = 'connection'
const DisconnectionEvent = 'disconnect'
const ErrorEvent = 'error'

const CycleDataWsEvent = '/data/cycle'
const ReceiptDataWsEvent = '/data/receipt'

const registeredDistributors = new Map<string, Socket>()

let RECEIPT_COUNTER = 0

export const setupDistributorSender = (): void => {
  const socketServer = new Server()

  socketServer.on(ConnectionEvent, (socket) => {
    console.log(`New distributor registered ${socket.id}`)
    registeredDistributors.set(socket.id, socket)
    socket.on(DisconnectionEvent, () => {
      console.log(`Distributor ${socket.id} disconnected`)
      registeredDistributors.delete(socket.id)
    })
    socket.on(ErrorEvent, (err) => {
      console.log(`Distributor ${socket.id} error: ${err}. Disconnecting...`)
      registeredDistributors.delete(socket.id)
      socket.disconnect()
    })
  })

  socketServer.listen(Number(CONFIG.port.collector))
  console.log(`Distributor sender listening on port ${CONFIG.port.collector}`)
}

export const forwardCycleData = async (data: Data): Promise<void> => {
  for (const socket of registeredDistributors.values()) {
    socket.emit(CycleDataWsEvent, data)
  }
  console.log(`Forwarded cycle data to ${registeredDistributors.size} distributors`)
}

export const forwardReceiptData = async (data: Data): Promise<void> => {
  RECEIPT_COUNTER++
  for (const socket of registeredDistributors.values()) {
    socket.emit(ReceiptDataWsEvent, data)
  }
  console.log(`Forwarded receipt data to ${registeredDistributors.size} distributors`)
  console.log('RECEIPTS SENT ->', RECEIPT_COUNTER)
}

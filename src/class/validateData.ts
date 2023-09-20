import * as crypto from '@shardus/crypto-utils'
import * as utils from '../utils'
import { config as CONFIG } from '../config'
import { insertOrUpdateCycle } from '../storage/cycle'
import { processReceiptData } from '../storage/receipt'
import { processOriginalTxData } from '../storage/originalTxData'
import { CycleLogWriter, ReceiptLogWriter, OriginalTxDataLogWriter } from './DataLogWriter'

export interface Data {
  receipts: any[]
  cycles: any[]
  originalTxsData: any[]
  sign: {
    owner: string
    sig: string
  }
}

export async function validateData(data: Data): Promise<void> {
  let err = utils.validateTypes(data, {
    sign: 'o',
    receipts: 'a?',
    cycles: 'a?',
    originalTxsData: 'a?',
  })
  if (err) {
    return
  }
  err = utils.validateTypes(data.sign, { owner: 's', sig: 's' })
  if (err) {
    return
  }
  if (data.sign.owner !== CONFIG.distributorInfo.publicKey) {
    console.log('Data received from distributor has invalid key')
    return
  }
  if (!crypto.verifyObj(data)) {
    console.log('Data received from distributor has invalid signature')
    return
  }
  if (!data.receipts && !data.cycles && !data.originalTxsData) {
    console.log('Data received from distributor is invalid')
    return
  }
  const { receipts, cycles, originalTxsData } = data
  if (cycles)
    for (const cycle of cycles) {
      if (!cycle.cycleRecord || !cycle.cycleMarker || cycle.counter < 0) {
        console.log('Invalid Cycle Received', cycle)
        return
      }
      CycleLogWriter.writeLog([`${JSON.stringify(cycle)}\n`])
      await insertOrUpdateCycle(cycle)
    }
  if (receipts) {
    ReceiptLogWriter.writeLog([`${JSON.stringify(receipts)}\n`])
    await processReceiptData(receipts)
  }
  if (originalTxsData) {
    OriginalTxDataLogWriter.writeLog([`${JSON.stringify(originalTxsData)}\n`])
    await processOriginalTxData(originalTxsData)
  }
}

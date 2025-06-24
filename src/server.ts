// require("dotenv").config();
import path from 'path'
import fs from 'fs'
import fastifyCors from '@fastify/cors'
import fastifyRateLimit from '@fastify/rate-limit'
import * as crypto from '@shardus/crypto-utils'
import Fastify, { FastifyRequest } from 'fastify'
import * as usage from './middleware/usage'
import * as Storage from './storage'
import { AccountDB, CycleDB, ReceiptDB, TransactionDB, OriginalTxDataDB } from './storage'
import {
  Account,
  AccountSearchType,
  AccountType,
  OriginalTxResponse,
  Transaction,
  TransactionSearchType,
  TransactionSearchParams,
  TransactionType,
  AccountSearchParams,
} from './types'
import { AccountResponse, ReceiptResponse, TransactionResponse } from './types'
import * as utils from './utils'
// config variables
import { config, envEnum } from './config'
import { Utils as StringUtils } from '@shardus/types'
import { healthCheckRouter } from './routes/healthCheck'

if (config.env == envEnum.DEV) {
  //default debug mode keys
  //  pragma: allowlist nextline secret
  config.USAGE_ENDPOINTS_KEY = 'ceba96f6eafd2ea59e68a0b0d754a939'
  config.collectorInfo.secretKey =
    //  pragma: allowlist nextline secret
    '7d8819b6fac8ba2fbac7363aaeb5c517e52e615f95e1a161d635521d5e4969739426b64e675cad739d69526bf7e27f3f304a8a03dca508a9180f01e9269ce447'
} else {
  // Pull in secrets
  const secretsPath = path.join(__dirname, '../../.secrets')
  const secrets = {}

  if (fs.existsSync(secretsPath)) {
    const lines = fs.readFileSync(secretsPath, 'utf-8').split('\n').filter(Boolean)

    lines.forEach((line) => {
      const [key, value] = line.split('=')
      secrets[key.trim()] = value.trim()
    })
  }
}

crypto.init(config.hashKey)
crypto.setCustomStringifier(StringUtils.safeStringify, 'shardus_safeStringify')

if (process.env.PORT) {
  config.port.server = process.env.PORT
}

console.log(process.argv)
const port = process.argv[2]
if (port) {
  config.port.server = port
}
console.log('Port', config.port.server)

export const addExitListeners = (): void => {
  process.on('SIGINT', async () => {
    console.log('Exiting on SIGINT')
    await Storage.closeDatabase()
    process.exit(0)
  })
  process.on('SIGTERM', async () => {
    console.log('Exiting on SIGTERM')
    await Storage.closeDatabase()
    process.exit(0)
  })
}

const start = async (): Promise<void> => {
  await Storage.initializeDB()
  addExitListeners()

  const server = Fastify({
    logger: config.fastifyDebugLog,
  })

  await server.register(fastifyCors)
  await server.register(fastifyRateLimit, {
    max: config.rateLimit,
    timeWindow: '1 minute',
    allowList: ['127.0.0.1', 'localhost'],
  })
  await server.register(healthCheckRouter)
  server.addContentTypeParser('application/json', { parseAs: 'string' }, (req, body, done) => {
    try {
      const jsonString = typeof body === 'string' ? body : body.toString('utf8')
      done(null, StringUtils.safeJsonParse(jsonString))
    } catch (err) {
      err.statusCode = 400
      done(err, undefined)
    }
  })

  server.setReplySerializer((payload) => {
    return StringUtils.safeStringify(payload)
  })

  // await server.register(fastifyMiddie)
  server.addHook('preHandler', usage.usageMiddleware)
  server.addHook('onError', usage.usageErrorMiddleware)
  server.post('/usage/enable', usage.usageEnableHandler)
  server.post('/usage/disable', usage.usageDisableHandler)
  server.get('/usage/metrics', usage.usageMetricsHandler)

  server.get('/port', (req, reply) => {
    reply.send({ port: config.port.server })
  })

  type CycleDataRequest = FastifyRequest<{
    Querystring: {
      count: string
      cycleNumber: string
      start: string
      end: string
      marker: string
    }
  }>

  server.get('/api/cycleinfo', async (_request: CycleDataRequest, reply) => {
    const err = utils.validateTypes(_request.query, {
      count: 's?',
      cycleNumber: 's?',
      start: 's?',
      end: 's?',
      marker: 's?',
    })
    if (err) {
      reply.send({ success: false, error: err })
      return
    }
    const query = _request.query
    // Check at least one of the query parameters is present
    if (!query.count && !query.cycleNumber && !query.start && !query.end && !query.marker) {
      reply.send({
        success: false,
        error: 'not specified which cycleinfo to query',
      })
    }
    let cycles = []
    if (query.count) {
      const count: number = parseInt(query.count)
      if (count <= 0 || Number.isNaN(count)) {
        reply.send({ success: false, error: 'Invalid count' })
        return
      }
      if (count > config.requestLimits.MAX_CYCLES_PER_REQUEST) {
        reply.send({
          success: false,
          error: `Maximum count is ${config.requestLimits.MAX_CYCLES_PER_REQUEST}`,
        })
        return
      }
      cycles = await CycleDB.queryLatestCycleRecords(count)
    } else if (query.cycleNumber) {
      const cycleNumber: number = parseInt(query.cycleNumber)
      if (cycleNumber < 0 || Number.isNaN(cycleNumber)) {
        reply.send({ success: false, error: 'Invalid cycleNumber' })
        return
      }
      const cycle = await CycleDB.queryCycleByCounter(cycleNumber)
      if (cycle) cycles = [cycle]
    } else if (query.start && query.end) {
      const from = parseInt(query.start)
      const to = parseInt(query.end)
      if (!(from >= 0 && to >= from) || Number.isNaN(from) || Number.isNaN(to)) {
        console.log('Invalid start and end counters for cycleinfo')
        reply.send({
          success: false,
          error: 'Invalid from and to counter for cycleinfo',
        })
        return
      }
      cycles = await CycleDB.queryCycleRecordsBetween(from, to)
      /* prettier-ignore */ if (config.verbose) console.log('cycles', cycles);
    } else if (query.marker) {
      const cycle = await CycleDB.queryCycleByMarker(query.marker)
      if (cycle) {
        cycles.push(cycle)
      }
    }
    const res = {
      success: true,
      cycles,
    }
    reply.send(res)
  })

  type AccountDataRequest = FastifyRequest<{
    Querystring: {
      count: string
      page: string
      accountSearchType: AccountSearchType
      startCycle: string
      endCycle: string
      accountId: string
    }
  }>

  server.get('/api/account', async (_request: AccountDataRequest, reply) => {
    const err = utils.validateTypes(_request.query, {
      count: 's?',
      page: 's?',
      accountSearchType: 's?',
      startCycle: 's?',
      endCycle: 's?',
      accountId: 's?',
    })
    if (err) {
      reply.send({ success: false, error: err })
      return
    }
    const query = _request.query
    // Check at least one of the query parameters is present
    if (
      !query.count &&
      !query.page &&
      !query.accountSearchType &&
      !query.startCycle &&
      !query.endCycle &&
      !query.accountId
    ) {
      reply.send({
        success: false,
        error: 'not specified which account to query',
      })
      return
    }
    const itemsPerPage = 10
    let totalPages = 0
    let totalAccounts = 0
    let accountSearchType: AccountSearchType
    let startCycle = 0
    let endCycle = 0
    let page = 1
    const res: AccountResponse = {
      success: true,
      accounts: [] as Account[],
    }
    if (query.accountSearchType) {
      if (
        typeof AccountType[query.accountSearchType] === 'undefined' &&
        typeof AccountSearchParams[query.accountSearchType] === 'undefined'
      ) {
        reply.send({ success: false, error: 'Invalid account search type' })
        return
      }
    }
    if (query.count) {
      const count: number = parseInt(query.count)
      if (count <= 0 || Number.isNaN(count)) {
        reply.send({ success: false, error: 'Invalid count' })
        return
      }
      if (count > config.requestLimits.MAX_ACCOUNTS_PER_REQUEST) {
        reply.send({
          success: false,
          error: `Maximum count is ${config.requestLimits.MAX_ACCOUNTS_PER_REQUEST}`,
        })
        return
      }
      res.accounts = await AccountDB.queryAccounts(0, count, null, null, accountSearchType)
      res.totalAccounts = await AccountDB.queryAccountCount(null, null, accountSearchType)
      reply.send(res)
      return
    } else if (query.accountId) {
      if (query.accountId.length !== 64) {
        reply.send({ success: false, error: 'Invalid account id' })
        return
      }
      const accountId = query.accountId.toLowerCase()
      const account = await AccountDB.queryAccountByAccountId(accountId)
      if (account) res.accounts = [account]
      reply.send(res)
      return
    }
    if (query.startCycle) {
      startCycle = parseInt(query.startCycle)
      if (startCycle < 0 || Number.isNaN(startCycle)) {
        reply.send({ success: false, error: 'Invalid start cycle number' })
        return
      }
      endCycle = startCycle
      if (query.endCycle) {
        endCycle = parseInt(query.endCycle)
        if (endCycle < 0 || Number.isNaN(endCycle) || endCycle < startCycle) {
          reply.send({ success: false, error: 'Invalid end cycle number' })
          return
        }
        if (endCycle - startCycle > config.requestLimits.MAX_BETWEEN_CYCLES_PER_REQUEST) {
          reply.send({
            success: false,
            error: `The cycle range is too big. Max cycle range is ${config.requestLimits.MAX_BETWEEN_CYCLES_PER_REQUEST} cycles.`,
          })
          return
        }
      }
    }
    if (query.page) {
      page = parseInt(query.page)
      if (page < 1 || Number.isNaN(page)) {
        reply.send({ success: false, error: 'Invalid page number' })
        return
      }
    }
    if (startCycle > 0 || endCycle > 0 || page > 0) {
      totalAccounts = await AccountDB.queryAccountCount(startCycle, endCycle, accountSearchType)
      res.totalAccounts = totalAccounts
    }
    totalPages = Math.ceil(totalAccounts / itemsPerPage)
    if (page > 1 && page > totalPages) {
      reply.send({
        success: false,
        error: 'Page no is greater than the totalPage',
      })
    }
    res.totalPages = totalPages
    if (totalAccounts > 0) {
      res.accounts = await AccountDB.queryAccounts(
        (page - 1) * itemsPerPage,
        itemsPerPage,
        null,
        null,
        accountSearchType
      )
    }
    reply.send(res)
  })

  type PollDataRequest = FastifyRequest<{
    Querystring: {
      account: string
      chatTimestamp: string
    }
  }>

  server.get('/api/poll', async (_request: PollDataRequest, reply) => {
    const err = utils.validateTypes(_request.query, {
      account: 's',
      chatTimestamp: 's',
    })
    if (err) {
      reply.send({ success: false, error: err })
      return
    }

    const query = _request.query
    const accountId = query.account.toLowerCase()
    const chatTimestamp = parseInt(query.chatTimestamp)

    // Validate account ID format
    if (accountId.length !== 64) {
      reply.send({ success: false, reason: 'Invalid account id' })
      return
    }

    // Validate timestamp
    if (isNaN(chatTimestamp) || chatTimestamp < 0) {
      reply.send({ success: false, reason: 'Invalid chatTimestamp' })
      return
    }

    // Check if account exists
    let account: Account
    let currentChatTimestamp = 0
    account = await AccountDB.queryAccountByAccountId(accountId)
    if (!account) {
      reply.send({ success: false, reason: 'account not found' })
      return
    } else {
      currentChatTimestamp = account.data?.data?.chatTimestamp
      if (currentChatTimestamp && currentChatTimestamp === chatTimestamp) {
        return reply.send({ success: true, chatTimestamp: currentChatTimestamp })
      }
    }

    const startTime = Date.now()
    const timeoutMs = 120 * 1000 // 120 seconds
    const checkIntervalMs = 1000 // 1 second

    while (Date.now() - startTime < timeoutMs) {
      account = await AccountDB.queryAccountByAccountId(accountId)

      currentChatTimestamp = account.data?.data?.chatTimestamp
      if (currentChatTimestamp && currentChatTimestamp !== chatTimestamp) {
        return reply.send({ success: true, chatTimestamp: currentChatTimestamp })
      }

      await utils.sleep(checkIntervalMs)
    }

    return reply.send({ success: false, reason: 'no change' })
  })

  type TransactionDataRequest = FastifyRequest<{
    Querystring: {
      count: string
      page: string
      txSearchType: string
      startCycle: string
      endCycle: string
      accountId: string
      txId: string
      startTimestamp: string
      endTimestamp: string
      appReceiptId: string
    }
  }>

  server.get('/api/transaction', async (_request: TransactionDataRequest, reply) => {
    const err = utils.validateTypes(_request.query, {
      count: 's?',
      page: 's?',
      accountId: 's?',
      txSearchType: 's?',
      startCycle: 's?',
      endCycle: 's?',
      txId: 's?',
      startTimestamp: 's?',
      endTimestamp: 's?',
      appReceiptId: 's?',
    })
    if (err) {
      reply.send({ success: false, error: err })
      return
    }
    /* prettier-ignore */ if (config.verbose) console.log('Request', _request.query);
    const query = _request.query
    // Check at least one of the query parameters is present
    if (
      !query.count &&
      !query.page &&
      !query.accountId &&
      !query.txSearchType &&
      !query.startCycle &&
      !query.endCycle &&
      !query.txId &&
      !query.startTimestamp &&
      !query.endTimestamp &&
      !query.appReceiptId
    ) {
      reply.send({
        success: false,
        reason: 'Not specified which transaction to query',
      })
      return
    }
    const itemsPerPage = 10
    let totalPages = 0
    let totalTransactions = 0
    let txSearchType: TransactionSearchType
    let startCycle = 0
    let endCycle = 0
    let startTimestamp = 0
    let endTimestamp = 0
    let page = 1
    let accountId = ''
    const res: TransactionResponse = {
      success: true,
      transactions: [] as Transaction[],
    }
    if (query.txSearchType) {
      txSearchType = query.txSearchType as TransactionSearchType
      // Check if the parsed value is a valid enum value
      if (
        typeof TransactionType[txSearchType] === 'undefined' &&
        typeof TransactionSearchParams[txSearchType] === 'undefined'
      ) {
        reply.send({ success: false, error: 'Invalid transaction search type' })
        return
      }
    }
    if (query.count) {
      const count: number = parseInt(query.count)
      if (count <= 0 || Number.isNaN(count)) {
        reply.send({ success: false, error: 'Invalid count' })
        return
      }
      if (count > config.requestLimits.MAX_TRANSACTIONS_PER_REQUEST) {
        reply.send({
          success: false,
          error: `Maximum count is ${config.requestLimits.MAX_TRANSACTIONS_PER_REQUEST}`,
        })
        return
      }
      res.transactions = await TransactionDB.queryTransactions(0, count, txSearchType)
      res.totalTransactions = await TransactionDB.queryTransactionCount(txSearchType)
      reply.send(res)
      return
    } else if (query.txId) {
      const txId = query.txId.toLowerCase()
      if (txId.length !== 64) {
        reply.send({ success: false, error: 'Invalid transaction id' })
        return
      }
      const transactions = await TransactionDB.queryTransactionByTxId(txId)
      if (transactions) res.transactions = [transactions]
      reply.send(res)
      return
    } else if (query.appReceiptId) {
      const appReceiptId = query.appReceiptId.toLowerCase()
      if (appReceiptId.length !== 64) {
        reply.send({ success: false, error: 'Invalid app receipt id' })
        return
      }
      // const transactions = await TransactionDB.queryTransactionByAppReceiptId(appReceiptId)
      const transaction = await TransactionDB.queryTransactionByTxId(appReceiptId)
      reply.send({ transaction: transaction ? transaction.data : null })
      return
    }
    if (query.accountId) {
      accountId = query.accountId.toLowerCase()
      if (accountId.length !== 64) {
        reply.send({ success: false, error: 'Invalid account id' })
        return
      }
    }
    if (query.startCycle) {
      startCycle = parseInt(query.startCycle)
      if (startCycle < 0 || Number.isNaN(startCycle)) {
        reply.send({ success: false, error: 'Invalid start cycle number' })
        return
      }
      endCycle = startCycle
      if (query.endCycle) {
        endCycle = parseInt(query.endCycle)
        if (endCycle < 0 || Number.isNaN(endCycle) || endCycle < startCycle) {
          reply.send({ success: false, error: 'Invalid end cycle number' })
          return
        }
        if (endCycle - startCycle > config.requestLimits.MAX_BETWEEN_CYCLES_PER_REQUEST) {
          reply.send({
            success: false,
            error: `The cycle range is too big. Max cycle range is ${config.requestLimits.MAX_BETWEEN_CYCLES_PER_REQUEST} cycles.`,
          })
          return
        }
      }
    }
    if (query.startTimestamp) {
      startTimestamp = parseInt(query.startTimestamp)
      if (startTimestamp < 0 || Number.isNaN(startTimestamp)) {
        reply.send({ success: false, error: 'Invalid start timestamp' })
        return
      }
      endTimestamp = startTimestamp
      if (query.endTimestamp) {
        endTimestamp = parseInt(query.endTimestamp)
        if (endTimestamp < 0 || Number.isNaN(endTimestamp) || endTimestamp < startTimestamp) {
          reply.send({ success: false, error: 'Invalid end timestamp' })
          return
        }
      }
    }
    if (query.page) {
      page = parseInt(query.page)
      if (page < 1 || Number.isNaN(page)) {
        reply.send({ success: false, error: 'Invalid page number' })
        return
      }
    }
    if (accountId || startCycle > 0 || endCycle > 0 || page > 0 || txSearchType) {
      totalTransactions = await TransactionDB.queryTransactionCount(
        txSearchType,
        accountId,
        startCycle,
        endCycle,
        startTimestamp,
        endTimestamp
      )
      res.totalTransactions = totalTransactions
    }
    totalPages = Math.ceil(totalTransactions / itemsPerPage)
    if (page > 1 && page > totalPages) {
      reply.send({
        success: false,
        error: 'Page no is greater than the totalPage',
      })
    }
    res.totalPages = totalPages
    if (totalTransactions > 0) {
      res.transactions = await TransactionDB.queryTransactions(
        (page - 1) * itemsPerPage,
        itemsPerPage,
        txSearchType,
        accountId,
        startCycle,
        endCycle,
        startTimestamp,
        endTimestamp
      )
    }
    reply.send(res)
  })

  type ReceiptDataRequest = FastifyRequest<{
    Querystring: {
      count: string
      page: string
      txId: string
      startCycle: string
      endCycle: string
      tally: string
    }
  }>

  server.get('/api/receipt', async (_request: ReceiptDataRequest, reply) => {
    const err = utils.validateTypes(_request.query, {
      count: 's?',
      page: 's?',
      txId: 's?',
      startCycle: 's?',
      endCycle: 's?',
      tally: 's?',
    })
    if (err) {
      reply.send({ success: false, error: err })
      return
    }
    /* prettier-ignore */ if (config.verbose) console.log('Request', _request.query);
    const query = _request.query
    // Check at least one of the query parameters is present
    if (!query.count && !query.page && !query.txId && !query.startCycle && !query.endCycle && !query.tally) {
      reply.send({
        success: false,
        reason: 'Not specified which receipt to query',
      })
      return
    }
    const itemsPerPage = 10
    let totalPages = 0
    let totalReceipts = 0
    let page = 1
    let startCycle = 0
    let endCycle = 0
    const res: ReceiptResponse = {
      success: true,
      receipts: [],
    }
    if (query.count) {
      const count: number = parseInt(query.count)
      if (count <= 0 || Number.isNaN(count)) {
        reply.send({ success: false, error: 'Invalid count' })
        return
      }
      if (count > config.requestLimits.MAX_RECEIPTS_PER_REQUEST) {
        reply.send({
          success: false,
          error: `Maximum count is ${config.requestLimits.MAX_RECEIPTS_PER_REQUEST}`,
        })
        return
      }
      res.receipts = await ReceiptDB.queryReceipts(0, count)
      res.totalReceipts = await ReceiptDB.queryReceiptCount()
      reply.send(res)
      return
    } else if (query.txId) {
      const txId: string = query.txId.toLowerCase()
      if (txId.length !== 64) {
        reply.send({ success: false, error: 'Invalid txId' })
        return
      }
      const receipts = await ReceiptDB.queryReceiptByReceiptId(txId)
      if (receipts) res.receipts = [receipts]
      reply.send(res)
      return
    }
    if (query.startCycle) {
      startCycle = parseInt(query.startCycle)
      if (startCycle < 0 || Number.isNaN(startCycle)) {
        reply.send({ success: false, error: 'Invalid start cycle number' })
        return
      }
      endCycle = startCycle
      if (query.endCycle) {
        endCycle = parseInt(query.endCycle)
        if (endCycle < 0 || Number.isNaN(endCycle) || endCycle < startCycle) {
          reply.send({ success: false, error: 'Invalid end cycle number' })
          return
        }
        if (endCycle - startCycle > config.requestLimits.MAX_BETWEEN_CYCLES_PER_REQUEST) {
          reply.send({
            success: false,
            error: `The cycle range is too big. Max cycle range is ${config.requestLimits.MAX_BETWEEN_CYCLES_PER_REQUEST} cycles.`,
          })
          return
        }
      }
    }
    if (query.tally === 'true') {
      const totalReceipts = await ReceiptDB.queryReceiptCountByCycles(startCycle, endCycle)
      reply.send({ success: true, totalReceipts })
      return
    }
    if (query.page) {
      page = parseInt(query.page)
      if (page < 1 || Number.isNaN(page)) {
        reply.send({ success: false, error: 'Invalid page number' })
        return
      }
    }
    if (startCycle > 0 || endCycle > 0 || page > 0) {
      totalReceipts = await ReceiptDB.queryReceiptCount(startCycle, endCycle)
      res.totalReceipts = totalReceipts
    }
    totalPages = Math.ceil(totalReceipts / itemsPerPage)
    if (page > 1 && page > totalPages) {
      reply.send({
        success: false,
        error: 'Page no is greater than the totalPage',
      })
    }
    res.totalPages = totalPages
    if (totalReceipts > 0) {
      res.receipts = await ReceiptDB.queryReceipts(
        (page - 1) * itemsPerPage,
        itemsPerPage,
        startCycle,
        endCycle
      )
    }
    reply.send(res)
  })

  type OriginalTxDataRequest = FastifyRequest<{
    Querystring: {
      count: string
      page: string
      txId: string
      accountId: string
      startCycle: string
      endCycle: string
      tally: string
    }
  }>

  server.get('/api/originalTx', async (_request: OriginalTxDataRequest, reply) => {
    const err = utils.validateTypes(_request.query, {
      count: 's?',
      page: 's?',
      txId: 's?',
      accountId: 's?',
      startCycle: 's?',
      endCycle: 's?',
      tally: 's?',
    })
    if (err) {
      reply.send({ success: false, error: err })
      return
    }
    /* prettier-ignore */ if (config.verbose) console.log('Request', _request.query);
    const query = _request.query
    // Check at least one of the query parameters is present
    if (
      !query.count &&
      !query.page &&
      !query.txId &&
      !query.accountId &&
      !query.startCycle &&
      !query.endCycle &&
      !query.tally
    ) {
      reply.send({
        success: false,
        reason: 'Not specified which original tx to query',
      })
      return
    }
    const itemsPerPage = 10
    let totalPages = 0
    let totalOriginalTxs = 0
    let page = 1
    let startCycle = 0
    let endCycle = 0
    let accountId = ''
    const res: OriginalTxResponse = {
      success: true,
      originalTxs: [],
    }
    if (query.count) {
      const count: number = parseInt(query.count)
      if (count <= 0 || Number.isNaN(count)) {
        reply.send({ success: false, error: 'Invalid count' })
        return
      }
      if (count > config.requestLimits.MAX_ORIGINAL_TXS_PER_REQUEST) {
        reply.send({
          success: false,
          error: `Maximum count is ${config.requestLimits.MAX_ORIGINAL_TXS_PER_REQUEST}`,
        })
        return
      }
      res.originalTxs = await OriginalTxDataDB.queryOriginalTxsData(0, count)
      res.totalOriginalTxs = await OriginalTxDataDB.queryOriginalTxDataCount()
      reply.send(res)
      return
    } else if (query.txId) {
      const txId: string = query.txId.toLowerCase()
      if (txId.length !== 64) {
        reply.send({ success: false, error: 'Invalid txId' })
        return
      }
      const originalTxs = await OriginalTxDataDB.queryOriginalTxDataByTxId(txId)
      if (originalTxs) res.originalTxs = [originalTxs]
      reply.send(res)
      return
    }
    if (query.accountId) {
      accountId = query.accountId.toLowerCase()
      if (accountId.length !== 64) {
        reply.send({ success: false, error: 'Invalid account id' })
        return
      }
    }
    if (query.startCycle) {
      startCycle = parseInt(query.startCycle)
      if (startCycle < 0 || Number.isNaN(startCycle)) {
        reply.send({ success: false, error: 'Invalid start cycle number' })
        return
      }
      endCycle = startCycle
      if (query.endCycle) {
        endCycle = parseInt(query.endCycle)
        if (endCycle < 0 || Number.isNaN(endCycle) || endCycle < startCycle) {
          reply.send({ success: false, error: 'Invalid end cycle number' })
          return
        }
        if (endCycle - startCycle > config.requestLimits.MAX_BETWEEN_CYCLES_PER_REQUEST) {
          reply.send({
            success: false,
            error: `The cycle range is too big. Max cycle range is ${config.requestLimits.MAX_BETWEEN_CYCLES_PER_REQUEST} cycles.`,
          })
          return
        }
      }
    }
    if (query.tally === 'true') {
      const totalOriginalTxs = await OriginalTxDataDB.queryOriginalTxDataCountByCycles(startCycle, endCycle)
      reply.send({ success: true, totalOriginalTxs })
      return
    }
    if (query.page) {
      page = parseInt(query.page)
      if (page < 1 || Number.isNaN(page)) {
        reply.send({ success: false, error: 'Invalid page number' })
        return
      }
    }
    if (accountId || startCycle > 0 || endCycle > 0 || page > 0) {
      totalOriginalTxs = await OriginalTxDataDB.queryOriginalTxDataCount(accountId, startCycle, endCycle)
      res.totalOriginalTxs = totalOriginalTxs
    }
    totalPages = Math.ceil(totalOriginalTxs / itemsPerPage)
    if (page > 1 && page > totalPages) {
      reply.send({
        success: false,
        error: 'Page no is greater than the totalPage',
      })
    }
    res.totalPages = totalPages
    if (totalOriginalTxs > 0) {
      res.originalTxs = await OriginalTxDataDB.queryOriginalTxsData(
        (page - 1) * itemsPerPage,
        itemsPerPage,
        accountId,
        startCycle,
        endCycle
      )
    }
    reply.send(res)
  })

  server.get('/totalData', async (_request, reply) => {
    interface TotalDataResponse {
      totalCycles: number
      totalAccounts?: number
      totalTransactions?: number
      totalReceipts: number
      totalOriginalTxs: number
    }

    const res: TotalDataResponse = {
      totalCycles: 0,
      totalReceipts: 0,
      totalOriginalTxs: 0,
    } // Initialize 'res' with an empty object

    res.totalCycles = await CycleDB.queryCycleCount()
    if (config.processData.indexReceipt) {
      res.totalAccounts = await AccountDB.queryAccountCount()
      res.totalTransactions = await TransactionDB.queryTransactionCount()
    }
    res.totalReceipts = await ReceiptDB.queryReceiptCount()
    res.totalOriginalTxs = await OriginalTxDataDB.queryOriginalTxDataCount()
    reply.send(res)
  })

  server.listen(
    {
      port: Number(config.port.server),
      host: '0.0.0.0',
    },
    async (err) => {
      if (err) {
        server.log.error(err)
        console.log(err)
        throw err
      }
      console.log('Server is listening on port:', config.port.server)
    }
  )
}

start()

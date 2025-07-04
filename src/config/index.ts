/* eslint-disable security/detect-object-injection */
import { readFileSync } from 'fs'
import merge from 'deepmerge'
import minimist from 'minimist'
import { join } from 'path'

export const envEnum = {
  DEV: 'development',
  PROD: 'production',
}

export enum collectorMode {
  WS = 'WS',
  MQ = 'MQ',
}

export interface Config {
  env: string
  host: string
  dataLogWrite: boolean
  dataLogWriter: {
    dirName: string
    maxLogFiles: number
    maxReceiptEntries: number
    maxCycleEntries: number
    maxOriginalTxEntries: number
  }
  collectorInfo: {
    publicKey: string
    secretKey: string
  }
  hashKey: string
  COLLECTOR_DB_DIR_PATH: string // Collectot DB folder name and path
  COLLECTOR_DATA: {
    cycleDB: string
    accountDB: string
    transactionDB: string
    receiptDB: string
    originalTxDataDB: string
    accountHistoryStateDB: string
  }
  port: {
    server: string
    collector: string
  }
  collectorSockerServer: {
    enabled: boolean
    forwardReceipt: boolean
    forwardAccount: boolean
    forwardTransaction: boolean
    forwardAppReceipt: boolean
  }
  distributorInfo: {
    ip: string
    port: string
    publicKey: string
  }
  verbose: boolean
  fastifyDebugLog: boolean
  rateLimit: number
  patchData: boolean
  USAGE_ENDPOINTS_KEY: string
  DISTRIBUTOR_RECONNECT_INTERVAL: number
  CONNECT_TO_DISTRIBUTOR_MAX_RETRY: number
  processData: {
    indexReceipt: boolean
    indexOriginalTxData: boolean
  }
  saveAccountHistoryState: boolean
  collectorMode: string
  storeReceiptBeforeStates: boolean
  requestLimits: {
    MAX_RECEIPTS_PER_REQUEST: number
    MAX_ORIGINAL_TXS_PER_REQUEST: number
    MAX_CYCLES_PER_REQUEST: number
    MAX_ACCOUNTS_PER_REQUEST: number
    MAX_TRANSACTIONS_PER_REQUEST: number
    MAX_BETWEEN_CYCLES_PER_REQUEST: number
    MAX_ACCOUNT_HISTORY_STATES_PER_REQUEST: number
  }
}

let config: Config = {
  env: process.env.SHARDEUM_COLLECTOR_MODE || envEnum.PROD, //default safe if env is not set
  host: process.env.HOST || '127.0.0.1',
  dataLogWrite: false,
  dataLogWriter: {
    dirName: 'data-logs',
    maxLogFiles: 10,
    maxReceiptEntries: 1000, // This value should be equivalent to the max TPS experiened by the network.
    maxCycleEntries: 1000,
    maxOriginalTxEntries: 1000, // This value should be equivalent to the max TPS experiened by the network.
  },
  collectorInfo: {
    publicKey: '',
    secretKey: '',
  },
  hashKey: '69fa4195670576c0160d660c3be36556ff8d504725be8a59b5a96509e0c994bc',
  COLLECTOR_DB_DIR_PATH: 'collector-db',
  COLLECTOR_DATA: {
    cycleDB: 'cycles.sqlite3',
    accountDB: 'accounts.sqlite3',
    transactionDB: 'transactions.sqlite3',
    receiptDB: 'receipts.sqlite3',
    originalTxDataDB: 'originalTxsData.sqlite3',
    accountHistoryStateDB: 'accountHistoryState.sqlite3',
  },
  port: {
    server: process.env.PORT || '6101',
    collector: process.env.COLLECTOR_PORT || '4444',
  },
  collectorSockerServer: {
    enabled: true,
    forwardReceipt: false,
    forwardAccount: false,
    forwardTransaction: false,
    forwardAppReceipt: true,
  },
  distributorInfo: {
    ip: process.env.DISTRIBUTOR_IP || '127.0.0.1',
    port: process.env.DISTRIBUTOR_PORT || '6100',
    publicKey: '',
  },
  verbose: false,
  fastifyDebugLog: false,
  rateLimit: 100,
  patchData: false,
  USAGE_ENDPOINTS_KEY: '',
  processData: {
    indexReceipt: true,
    indexOriginalTxData: true,
  },
  saveAccountHistoryState: true,
  DISTRIBUTOR_RECONNECT_INTERVAL: 10_000,
  CONNECT_TO_DISTRIBUTOR_MAX_RETRY: 10,
  collectorMode: process.env.COLLECTOR_MODE || collectorMode.WS.toString(),
  storeReceiptBeforeStates: true,
  requestLimits: {
    MAX_RECEIPTS_PER_REQUEST: 100,
    MAX_ORIGINAL_TXS_PER_REQUEST: 100,
    MAX_CYCLES_PER_REQUEST: 100,
    MAX_ACCOUNTS_PER_REQUEST: 100,
    MAX_TRANSACTIONS_PER_REQUEST: 100,
    MAX_BETWEEN_CYCLES_PER_REQUEST: 100,
    MAX_ACCOUNT_HISTORY_STATES_PER_REQUEST: 100,
  },
}

let DISTRIBUTOR_URL = `http://${config.distributorInfo.ip}:${config.distributorInfo.port}`

// Override default config params from config file, env vars, and cli args
export function overrideDefaultConfig(env: NodeJS.ProcessEnv, args: string[]): void {
  const file = join(process.cwd(), 'config.json')
  // Override config from config file
  try {
    // eslint-disable-next-line security/detect-non-literal-fs-filename
    const fileConfig = JSON.parse(readFileSync(file, { encoding: 'utf8' }))
    const overwriteMerge = (target: [], source: []): [] => source
    config = merge(config, fileConfig, { arrayMerge: overwriteMerge })
  } catch (err) {
    if (err && err.code !== 'ENOENT') {
      console.warn('Failed to parse config file:', err)
    }
  }

  // Override config from env vars
  for (const param in config) {
    if (env[param]) {
      switch (typeof config[param]) {
        case 'number': {
          config[param] = Number(env[param])
          break
        }
        case 'string': {
          config[param] = String(env[param])
          break
        }
        case 'object': {
          try {
            const parameterStr = env[param]
            if (parameterStr) {
              const parameterObj = JSON.parse(parameterStr)
              config[param] = parameterObj
            }
          } catch (e) {
            console.error(e)
            console.error('Unable to JSON parse', env[param])
          }
          break
        }
        case 'boolean': {
          config[param] = String(env[param]).toLowerCase() === 'true'
          break
        }
      }
    }
  }

  // Override config from cli args
  const parsedArgs = minimist(args.slice(2))
  for (const param of Object.keys(config)) {
    if (parsedArgs[param]) {
      switch (typeof config[param]) {
        case 'number': {
          config[param] = Number(parsedArgs[param])
          break
        }
        case 'string': {
          config[param] = String(parsedArgs[param])
          break
        }
        case 'boolean': {
          if (typeof parsedArgs[param] === 'boolean') {
            config[param] = parsedArgs[param]
          } else {
            config[param] = String(parsedArgs[param]).toLowerCase() === 'true'
          }
          break
        }
      }
    }
  }

  DISTRIBUTOR_URL = `http://${config.distributorInfo.ip}:${config.distributorInfo.port}`
}

export { config, DISTRIBUTOR_URL }

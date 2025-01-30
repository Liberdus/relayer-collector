/** Same as type AccountsCopy in the shardus core */
export type AccountsCopy = {
  accountId: string
  data: any // eslint-disable-line @typescript-eslint/no-explicit-any
  timestamp: number
  hash: string
  cycleNumber: number
  isGlobal: boolean
}

export interface Account extends AccountsCopy {
  accountType?: AccountType
}

export enum AccountType {}

export enum AccountSearchParams {
  'all', // All Accounts Type
  // e.g 'UserAndNodeAccounts' for User and Node Accounts
}

export type AccountSearchType = AccountType | AccountSearchParams

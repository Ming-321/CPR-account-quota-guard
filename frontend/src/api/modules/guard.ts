import { request } from '../request'

export type Rule = { mode: 'fixed', threshold: number } | { mode: 'random', min: number, max: number }
export interface Sample { observed: number, reset: number | null, used: number }
export interface Guard {
  enabled: boolean
  configured: Rule
  current: Rule
  threshold: number | null
  cycle: Sample | null
  latest: Sample | null
  blocked: boolean
}
export interface Row { guard: Guard, status: string, reason: string | null }
export interface Account { account_id: string, name: string, email: string | null, enabled: boolean }
export interface Snapshot { version: number | null, accounts: Account[], guards: Record<string, Row> }
export const load = () => request<Snapshot>('GET', 'state')
export function save(version: number | null, account_id: string, enabled: boolean, rule: Rule) {
  return request<Snapshot>('POST', 'settings', { version, account_id, enabled, rule })
}

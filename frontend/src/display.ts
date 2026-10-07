import type { Rule } from './api/modules/guard'

export function ruleText(rule: Rule): string {
  return rule.mode === 'fixed' ? `固定 ${rule.threshold}%` : `随机 ${rule.min}%–${rule.max}%`
}
export function ruleError(mode: string, threshold: number, min: number, max: number): string {
  const valid = (n: number) => Number.isInteger(n) && n >= 1 && n <= 100
  if (mode === 'fixed')
    return valid(threshold) ? '' : '阈值须为 1–100 的整数'
  return valid(min) && valid(max) && min <= max ? '' : '区间须为 1–100 的整数，且下限不大于上限'
}
export function date(value?: number | null): string {
  return value ? new Date(value).toLocaleString('zh-CN', { hour12: false }) : '暂无'
}

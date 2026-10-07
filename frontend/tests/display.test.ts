import assert from 'node:assert/strict'
import { test } from 'node:test'
import { ruleError, ruleText } from '../src/display.ts'

test('整数和区间输入边界', () => {
  assert.equal(ruleError('fixed', 15, 0, 0), '')
  assert.ok(ruleError('fixed', 15.5, 0, 0))
  assert.ok(ruleError('random', 0, 20, 10))
  assert.equal(ruleError('random', 0, 100, 100), '')
  assert.equal(ruleText({ mode: 'random', min: 10, max: 20 }), '随机 10%–20%')
})

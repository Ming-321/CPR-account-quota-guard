<script setup lang="ts">
import type { Row, Rule, Snapshot } from './api/modules/guard'
import { BaseButton, BaseIconButton, BaseModal, BaseSelect, BaseSwitch, BaseTag } from '@codex-proxy/ui'
import { Plus, RefreshCw, Settings } from '@lucide/vue'
import { computed, onMounted, ref } from 'vue'
import { load, save } from './api/modules/guard'
import { date, ruleError, ruleText } from './display'

const data = ref<Snapshot>()
const busy = ref(false)
const error = ref('')
const notice = ref('')
const open = ref(false)
const editing = ref(false)
const accountId = ref('')
const enabled = ref(true)
const mode = ref('fixed')
const threshold = ref(15)
const min = ref(10)
const max = ref(20)
const formError = ref('')
const rows = computed(() => Object.entries(data.value?.guards || {}))
const accounts = computed(() => (data.value?.accounts || []).map(a => ({
  value: a.account_id,
  label: a.name || a.email || a.account_id,
  disabled: !editing.value && !!data.value?.guards[a.account_id],
})))
const account = (id: string) => data.value?.accounts.find(a => a.account_id === id)
const pending = (row: Row) => JSON.stringify(row.guard.configured) !== JSON.stringify(row.guard.current)
const tone = (row: Row) => !row.guard.enabled ? 'neutral' : row.guard.blocked ? 'danger' : row.status === '等待额度数据' ? 'warning' : 'success'
async function refresh(manual = true) {
  busy.value = true
  error.value = ''
  notice.value = ''
  try {
    data.value = await load()
    if (manual)
      notice.value = '列表已更新'
  }
  catch (e) { error.value = (e as Error).message }
  finally { busy.value = false }
}
function edit(id?: string) {
  editing.value = !!id
  accountId.value = id || ''
  const guard = id ? data.value?.guards[id]?.guard : undefined
  enabled.value = guard?.enabled ?? true
  const rule = guard?.configured
  mode.value = rule?.mode || 'fixed'
  threshold.value = rule?.mode === 'fixed' ? rule.threshold : 15
  min.value = rule?.mode === 'random' ? rule.min : 10
  max.value = rule?.mode === 'random' ? rule.max : 20
  formError.value = ''
  open.value = true
}
async function submit() {
  formError.value = accountId.value ? ruleError(mode.value, threshold.value, min.value, max.value) : '请选择账号'
  if (formError.value || busy.value || !data.value)
    return
  busy.value = true
  const rule: Rule = mode.value === 'fixed'
    ? { mode: 'fixed', threshold: threshold.value }
    : { mode: 'random', min: min.value, max: max.value }
  try {
    data.value = await save(data.value.version, accountId.value, enabled.value, rule)
    open.value = false
    notice.value = '设置已保存'
  }
  catch (e) { formError.value = (e as Error).message }
  finally { busy.value = false }
}
onMounted(() => refresh(false))
</script>

<template>
  <main>
    <div class="toolbar">
      <span class="muted">{{ rows.length }} 个账号</span>
      <div class="actions">
        <span class="tip-wrap">
          <BaseIconButton label="刷新列表" aria-describedby="refresh-tip" :disabled="busy" @click="refresh()">
            <RefreshCw :size="17" :class="{ spin: busy }" />
          </BaseIconButton>
          <span id="refresh-tip" role="tooltip" class="tooltip">刷新列表，仅读取CPR已有数据，不会请求上游</span>
        </span>
        <span class="tip-wrap">
          <BaseIconButton label="添加账号" aria-describedby="add-tip" :disabled="busy || !data" @click="edit()">
            <Plus :size="19" />
          </BaseIconButton>
          <span id="add-tip" role="tooltip" class="tooltip">添加账号</span>
        </span>
      </div>
    </div>
    <p v-if="error" role="alert" class="error">
      {{ error }}
    </p>
    <p v-if="notice" role="status" class="notice">
      {{ notice }}
    </p>
    <div v-if="!data && busy" class="empty">
      正在加载账号
    </div>
    <div v-else-if="data && !rows.length" class="empty">
      <p>尚未添加保护账号</p>
      <BaseButton @click="edit()">
        添加账号
      </BaseButton>
    </div>
    <section v-else class="account-table" aria-label="账号额度保护列表">
      <div class="table-head">
        <span>账号</span><span>周额度剩余</span><span>本周期阈值</span><span>保护状态</span><span />
      </div>
      <div v-for="[id, row] in rows" :key="id" class="account-row">
        <div class="identity">
          <strong>{{ account(id)?.name || '账号已删除' }}</strong>
          <small>{{ account(id)?.email || id }}</small>
          <small v-if="account(id) && !account(id)?.enabled" class="warning">宿主已停用</small>
        </div>
        <div class="cell" data-label="周额度剩余">
          <strong>{{ row.guard.latest ? `${Number((100 - row.guard.latest.used).toFixed(2))}%` : '暂无' }}</strong>
          <progress v-if="row.guard.latest" :value="100 - row.guard.latest.used" max="100" aria-label="周额度剩余" />
        </div>
        <div class="cell" data-label="本周期阈值">
          <strong>{{ row.guard.threshold == null ? '待确定' : `${row.guard.threshold}%` }}</strong>
          <small>{{ ruleText(row.guard.current) }}</small>
          <small v-if="pending(row)" class="warning">下周期：{{ ruleText(row.guard.configured) }}</small>
        </div>
        <div class="cell" data-label="保护状态">
          <BaseTag :type="tone(row)">
            {{ row.status }}
          </BaseTag>
          <details>
            <summary>详情</summary>
            <div class="detail">
              <p v-if="row.reason">
                {{ row.reason }}
              </p>
              <p>额度观测：{{ date(row.guard.latest?.observed) }}</p>
              <p>预计重置：{{ date(row.guard.latest?.reset) }}</p>
            </div>
          </details>
        </div>
        <BaseIconButton label="设置账号保护" title="设置账号保护" :disabled="busy" @click="edit(id)">
          <Settings :size="17" />
        </BaseIconButton>
      </div>
    </section>
  </main>
  <BaseModal v-model="open" :title="editing ? '账号保护设置' : '添加账号'" :dismissible="!busy" :draggable="false">
    <form class="form" @submit.prevent="submit">
      <label>账号</label>
      <BaseSelect v-model="accountId" :options="accounts" :disabled="editing || busy" placeholder="选择账号" />
      <BaseSwitch v-model="enabled" label="启用保护" show-label :disabled="busy" />
      <p v-if="!enabled" class="muted">
        关闭后解除本插件的拦截，保留当前周期阈值
      </p>
      <label>阈值模式</label>
      <BaseSelect v-model="mode" :options="[{ value: 'fixed', label: '固定阈值' }, { value: 'random', label: '每周期随机' }]" :disabled="busy" />
      <label v-if="mode === 'fixed'" class="field">剩余低于
        <span class="number"><input v-model.number="threshold" aria-label="固定阈值" type="number" min="1" max="100" step="1" :disabled="busy">%</span>
      </label>
      <div v-else class="range">
        <label class="field">下限<span class="number"><input v-model.number="min" aria-label="随机下限" type="number" min="1" max="100" step="1" :disabled="busy">%</span></label>
        <label class="field">上限<span class="number"><input v-model.number="max" aria-label="随机上限" type="number" min="1" max="100" step="1" :disabled="busy">%</span></label>
      </div>
      <p v-if="editing" class="muted">
        固定阈值修改立即生效，模式切换和随机区间修改在下个确认的周期生效
      </p>
      <p v-else-if="mode === 'random'" class="muted">
        每周期抽取一次，首次等待可识别的周额度周期
      </p>
      <p v-if="formError" role="alert" class="error">
        {{ formError }}
      </p>
      <div class="footer">
        <BaseButton :disabled="busy" @click="open = false">
          取消
        </BaseButton>
        <BaseButton variant="primary" :disabled="busy" @click="submit">
          {{ busy ? '正在保存' : '保存' }}
        </BaseButton>
      </div>
    </form>
  </BaseModal>
</template>

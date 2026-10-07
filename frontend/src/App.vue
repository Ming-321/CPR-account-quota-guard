<script setup lang="ts">
import type { Row, Rule, Snapshot } from './api/modules/guard'
import { BaseButton, BaseIconButton, BaseModal, BasePopover, BaseSelect, BaseSwitch, BaseTag } from '@codex-proxy/ui'
import { Clock3, Info, Plus, RefreshCw, Settings, ShieldCheck } from '@lucide/vue'
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
  label: a.name?.trim() || a.email?.trim() || a.account_id,
  disabled: !editing.value && !!data.value?.guards[a.account_id],
})))
const account = (id: string) => data.value?.accounts.find(a => a.account_id === id)
const accountName = (id: string) => account(id)?.name?.trim() || account(id)?.email?.trim() || (account(id) ? '未命名账号' : '账号已删除')
function accountEmail(id: string) {
  if (!account(id)?.name?.trim())
    return ''
  const email = account(id)?.email?.trim()
  return email && email.toLowerCase() !== accountName(id).toLowerCase() ? email : ''
}
const remaining = (row: Row) => row.guard.latest ? Number((100 - row.guard.latest.used).toFixed(2)) : null
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
      <div class="list-heading">
        <ShieldCheck :size="18" /><span>{{ rows.length }} 个保护账号</span><span v-if="notice" role="status" class="notice">{{ notice }}</span>
      </div>
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
          <strong>{{ accountName(id) }}</strong>
          <small v-if="accountEmail(id)">{{ accountEmail(id) }}</small>
          <small v-if="account(id) && !account(id)?.enabled" class="warning">宿主已停用</small>
        </div>
        <div class="cell" data-label="周额度剩余">
          <div class="quota" :class="{ 'quota-paused': row.guard.enabled && row.guard.blocked, 'quota-disabled': !row.guard.enabled }">
            <strong class="numeric">{{ remaining(row) == null ? '暂无数据' : `${remaining(row)}%` }}</strong>
            <div v-if="remaining(row) != null" class="quota-track" role="progressbar" aria-label="周额度剩余" :aria-valuenow="remaining(row)!" :aria-valuemin="0" :aria-valuemax="100" :aria-valuetext="`剩余 ${remaining(row)}%，保护阈值 ${row.guard.threshold ?? '待确定'}%`">
              <span class="quota-fill" :style="{ width: `${remaining(row)}%` }" />
              <span v-if="row.guard.threshold != null" class="quota-marker" :style="{ left: `${row.guard.threshold}%` }" :title="`保护阈值 ${row.guard.threshold}%`" />
            </div>
          </div>
        </div>
        <div class="cell" data-label="本周期阈值">
          <div class="threshold-line">
            <strong class="numeric">{{ row.guard.threshold == null ? '待确定' : `${row.guard.threshold}%` }}</strong>
            <BasePopover placement="bottom-end">
              <template #trigger="{ open: ruleOpen }">
                <BaseIconButton label="查看阈值规则" title="查看阈值规则" :aria-expanded="ruleOpen">
                  <Info :size="16" />
                </BaseIconButton>
              </template>
              <section class="quota-detail" aria-label="阈值规则">
                <div class="detail-heading">
                  本周期规则
                </div>
                <p class="detail-account">
                  {{ accountName(id) }}
                </p>
                <dl>
                  <dt>阈值模式</dt><dd>{{ row.guard.current.mode === 'fixed' ? '固定阈值' : '每周期随机' }}</dd>
                  <template v-if="row.guard.current.mode === 'random'">
                    <dt>随机区间</dt><dd>{{ row.guard.current.min }}%–{{ row.guard.current.max }}%</dd>
                  </template>
                </dl>
                <p v-if="pending(row)" class="detail-reason">
                  <Clock3 :size="14" />下周期：{{ ruleText(row.guard.configured) }}
                </p>
              </section>
            </BasePopover>
          </div>
        </div>
        <div class="cell" data-label="保护状态">
          <div class="status-line">
            <BaseTag :type="tone(row)" size="sm">
              {{ row.status }}
            </BaseTag>
            <BasePopover placement="bottom-end">
              <template #trigger="{ open: detailOpen }">
                <BaseIconButton label="查看额度详情" title="查看额度详情" :aria-expanded="detailOpen">
                  <Info :size="16" />
                </BaseIconButton>
              </template>
              <section class="quota-detail" aria-label="额度详情">
                <div class="detail-heading">
                  <span>额度详情</span><span class="muted">本地时间</span>
                </div>
                <p class="detail-account">
                  {{ accountName(id) }}
                </p>
                <dl><dt>额度更新时间</dt><dd>{{ date(row.guard.latest?.observed) }}</dd><dt>预计重置时间</dt><dd>{{ date(row.guard.latest?.reset) }}</dd></dl>
                <p v-if="row.reason" class="detail-reason">
                  <Clock3 :size="14" />{{ row.reason }}
                </p>
              </section>
            </BasePopover>
          </div>
        </div>
        <BaseIconButton label="设置账号保护" title="设置账号保护" :disabled="busy" @click="edit(id)">
          <Settings :size="17" />
        </BaseIconButton>
      </div>
    </section>
  </main>
  <BaseModal v-model="open" :title="editing ? '账号保护设置' : '添加账号'" :dismissible="!busy" :draggable="false">
    <form class="form" @submit.prevent="submit">
      <div v-if="editing" class="edit-identity">
        <span class="field-caption">保护账号</span><strong>{{ accountName(accountId) }}</strong><small v-if="accountEmail(accountId)">{{ accountEmail(accountId) }}</small>
      </div>
      <div v-else class="form-field">
        <label>账号</label><BaseSelect v-model="accountId" :options="accounts" filterable :disabled="busy" placeholder="选择账号名称或邮箱" aria-label="账号" />
      </div>
      <div class="enable-row">
        <div>
          <span class="field-title">账号保护</span><p class="muted">
            {{ enabled ? '低于阈值时拦截新请求' : '已解除拦截，保留当前周期阈值' }}
          </p>
        </div><BaseSwitch v-model="enabled" label="启用保护" :disabled="busy" />
      </div>
      <div class="form-field">
        <label>阈值模式</label><div class="mode-picker" role="group" aria-label="阈值模式">
          <button type="button" :aria-pressed="mode === 'fixed'" :disabled="busy" @click="mode = 'fixed'">
            固定阈值
          </button><button type="button" :aria-pressed="mode === 'random'" :disabled="busy" @click="mode = 'random'">
            每周期随机
          </button>
        </div>
      </div>
      <label v-if="mode === 'fixed'" class="field">剩余低于
        <span class="number"><input v-model.number="threshold" aria-label="固定阈值" type="number" min="1" max="100" step="1" :disabled="busy">%</span>
      </label>
      <div v-else class="range">
        <label class="field">下限<span class="number"><input v-model.number="min" aria-label="随机下限" type="number" min="1" max="100" step="1" :disabled="busy">%</span></label>
        <label class="field">上限<span class="number"><input v-model.number="max" aria-label="随机上限" type="number" min="1" max="100" step="1" :disabled="busy">%</span></label>
      </div>
      <p v-if="editing" class="form-help">
        固定阈值修改立即生效，模式切换和随机区间修改在下个确认的周期生效
      </p>
      <p v-else-if="mode === 'random'" class="form-help">
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

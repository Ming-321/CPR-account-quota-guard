# 开发与验证

## 结构

- `src/cycle.rs`：读取周窗口与识别周期
- `src/model.rs`：阈值、待生效配置与保护判断
- `src/engine.rs`：读写权威状态，保存成功后再使用新结果
- `src/host.rs`：只读账号/额度及插件私有状态
- `src/main.rs`：attempt 中间件与管理路由
- `frontend/`：独立 Vue 页面，通过 CPR 管理桥调用插件

一个进程内串行协调状态，调用下游前释放锁。不同进程代次用宿主状态版本防止覆盖，冲突时请求重新读取。管理保存使用页面版本，过期时要求刷新，不覆盖别人的设置。

插件没有定时任务，只在请求、页面读取和保存时消费已有事实。允许的业务回调是 `host.data.accounts.list`、`host.data.quota.get`、`host.state.get`、`host.state.put` 以及中间件的 `next`。

周期保留初始重置时间锚点，避免小幅漂移累计成新周期。提前重置要求稳定的重置时间及相隔至少 5 分钟、周用量发生变化的观测；无法确认时不放宽保护，但新的有效周用量增加可以触发拦截。整个账号的观测时间单独变化，不会确认周期。

## 打包

从官方 CPR 提交 `f174320e2ac8987578146d4684e69ad196db0286` 构建 `codex-proxy-plugin-cli`，使用生成的 `cpr-plugin`：

```bash
cpr-plugin package \
  --manifest plugin.json \
  --binary target/release/account-quota-guard \
  --target aarch64-unknown-linux-gnu \
  --resource-map web=frontend/dist \
  --output-dir dist
```

x86_64 在对应原生 Linux runner 构建，target 改为 `x86_64-unknown-linux-gnu`。CI 在两个架构上检查、测试和打包，版本 tag 通过后发布正式附件。

## 隔离验证

`validation/` 使用专用 PostgreSQL、Redis 和本地合成上游，不能连接生产配置。测试密码和合成 Key 只用于无外网的隔离环境，不用于部署。

- `upstream.py`：可控制额度的 HTTP/SSE/WS 合成上游，仅记录请求路径、账号代号和时间
- `scenarios.py`：真实安装后的边界、拦截、恢复、停启与并发场景
- `websocket.py`：同一 WebSocket 的后续请求及恢复
- `validation/audit-proxy.pl`：透明代理执行实际插件，只记录宿主回调方法名；不进入发行安装包

测试脚本读取 `CPR_TEST_BASE` 和 `CPR_TEST_IDS`。后者是测试账号映射文件，形如：

```json
{
  "A": {"account": "<测试账号 ID>", "group": "<测试组 ID>", "key": "<合成 Key>"},
  "B": {"account": "<测试账号 ID>", "group": "<测试组 ID>", "key": "<合成 Key>"}
}
```

每个 Key 只绑定对应账号所在的测试组。HTTP 场景使用账号的 HTTP 传输，WebSocket 场景改为优先 WebSocket。额度更新由合成响应或测试人员调用宿主原生刷新接口产生，插件自身不得调用刷新。

`scenarios.py` 对宿主选号前的 `account_capacity_unavailable` 做短暂等待；其他失败不会当作成功。单元测试另行验证状态版本冲突、写入失败和提前重置，不把合成宿主测试等同于真实上游验证。

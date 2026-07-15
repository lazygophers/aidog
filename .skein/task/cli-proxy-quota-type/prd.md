# cli_proxy quota 字段: NewAPI 显式标识 — PRD

## 目标
CliProxyProvider 加 `quota` JSON 列, 用户显式标识 provider 余额查询类型 (none/newapi)。test_cmd 按 quota.type 分流, 替代无脑 fallback, 避免 x.ai 等非 newapi 平台发无效 /api/usage/token/ 请求留 404 垃圾日志。

## 背景
- commit 01f0acbe 加 fallback: query_quota Unsupported → 试 query_quota_newapi。x.ai provider 触发 → 404 留 quota 日志 (request_id=9e0aec4fb88146f582d2936c47050288)
- platform 侧靠 Protocol::NewApi 分流; cli_proxy 无类型字段, fallback 无识别机制
- 用户裁: 加 quota_type 字段 (JSON 类型, 同 extra 模式)

## 设计
### DB (migration 048, 主库)
`cli_proxy_provider` 加列 `quota TEXT NOT NULL DEFAULT '{}'` (JSON 串, 仿 extra)。
JSON 结构: `{"type": "none" | "newapi"}` (默认 none)。未来可扩展 (balance_base_url 等)。

### Rust model (cli_proxy.rs)
- `CliProxyProvider` 加 `pub quota: String`
- `CreateCliProxyProvider` 加 `#[serde(default)] pub quota: String` (默认 "{}")

### db/cli_proxy.rs CRUD
- `CLI_PROXY_COLUMNS` 加 quota (列序: extra 后插)
- row.get 索引调整
- INSERT 加 quota 列 + 占位符
- UPDATE 加 quota

### test_cmd.rs 分流
读 `provider.quota` JSON → 解析 type:
- `newapi` → `query_quota_newapi(extra)`
- 其他/none → `query_quota` (原生 dispatch)
- **回退 01f0acbe 的无脑 fallback** (改为按 type 显式分流)

### 前端
- `cliProxy.ts` 类型加 `quota: string` (CliProxyProvider) / `quota?: string` (Create)
- `CliProxy.tsx` form 加 quota 类型 select (none/newapi), 存 JSON `{"type":...}`
- 列表卡片可选显 quota 类型徽标
- i18n label

## 边界
- migration 主库 (cli_proxy_provider 主库归属, 非 log.db)
- 向后兼容: 旧行 quota='{}' → type=none → 原生 dispatch (等同 fallback 前行为)
- 回退 01f0acbe fallback commit (无脑 fallback 删除, 按 type 分流替代)

## 验收标准
- [ ] migration 048 加 quota 列, 旧库升级幂等
- [ ] CRUD 读写 quota 列
- [ ] test_cmd: quota.type=newapi → query_quota_newapi; none → query_quota
- [ ] 无脑 fallback (01f0acbe) 移除
- [ ] 前端 form 可选 newapi, 存 JSON
- [ ] x.ai provider (quota.type=none) 测试不再发 /api/usage/token/ 请求
- [ ] cargo clippy + cargo test 过
- [ ] yarn build + check:i18n 过

## 索引
- task.json: `.skein/task/cli-proxy-quota-type/task.json`
- 前身: 01f0acbe (fallback, 本次回退)

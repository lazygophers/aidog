# 背景

Logs 详情页 Meta 概览区只展示「状态」(`detail.status_code`，回客户端响应状态)，未展示上游 HTTP 状态码；而 attempts 块以 `length > 1` 为门槛导致单平台一次成功请求永远看不到上游尝试。

数据模型已支持：`proxy_log.upstream_status_code` 列（src-tauri/src/gateway/db.rs:1988 / 迁移 src-tauri/migrations/001_init.sql:84）、`attempts` JSON 串（同 db.rs:2012 注释），详情 API `row_to_proxy_log` 已映射三字段（db.rs:1926 `upstream_status_code: row.get(15)`、db.rs:1929 `status_code: row.get(18)`、db.rs:1936 `attempts`）。前端 `ProxyLogDetail` 类型已声明 `upstream_status_code: number`（src/services/api.ts:649）+ `attempts: ProxyAttempt[]`（api.ts:660，注释「单平台一次成功时长度 1」）。**后端无需改。**

`RequestTabs` 上游 tab 的 `statusCode` 已经传入 `detail.upstream_status_code`（src/pages/Logs.tsx:383），即 tab 内容区已含；本次只补 Meta 概览 chip 与 attempts 渲染门槛。

用户已答 3 问收敛范围（见目标/非目标）。

# 目标

1. **详情 Meta 概览新增「上游状态」chip**：紧邻现有「状态」格（src/pages/Logs.tsx:315 之后），渲染 `detail.upstream_status_code`：
   - 2xx → success 色（与 status_code === 200 同色）
   - 非 2xx 且非 0 → danger 色
   - `0`（连接失败 / 超时 / 上游未捕获）→ 灰色 "-" 或复用 `t("logs.connFailed", "连接失败")`（遵循现有 attempts 行逻辑 Logs.tsx:355）
   - 缺失（null / undefined 旧数据兜底）→ "-"
2. **attempts 块去掉 `> 1` 门槛**：将条件 `detail.attempts && detail.attempts.length > 1`（Logs.tsx:325）改为 `>= 1`（或恒真 + 内部空数组兜底），使单次成功请求也展示那一行尝试时序。
3. **i18n 8 语言补 key**：`logs.upstreamStatus`（上游状态）+ `logs.notCaptured`（未捕获/兜底占位，若不复用 connFailed）。
4. **零回归保障**：列表表格、`RequestTabs`、`ProxyLogSummary` 类型保持不动。

# 非目标

- **不在列表表格展示 upstream_status_code**（用户已明确：详情可见即可）。
- 不改 DB schema / 详情 API（字段已返回）。
- 不改 `RequestTabs` 内部布局（已显示 statusCode）。
- 不改 attempts 元素结构（platform_name / status_code / error / duration_ms / ts 已足够）。
- 不引入新的状态码语义分类（沿用现有 2xx / 0 / 其它三态）。

# 验收标准

- [ ] AC1：详情页 Meta grid 在「状态」格之后出现「上游状态」格，显示 `detail.upstream_status_code`；2xx 成功色、非 2xx 失败色、0 或缺失显示 "-" / 「连接失败」。
- [ ] AC2：单平台一次成功请求（`attempts.length === 1`）的详情页能看到 Attempts 块渲染 1 行（含 #1 / platform_name / status_code / duration_ms / error）。
- [ ] AC3：`src/pages/Logs.tsx` 仅改动 Meta 区 1 格 + attempts 条件，列表表格零 diff。
- [ ] AC4：`src/locales/{zh-CN,en-US,ar-SA,fr-FR,de-DE,ru-RU,ja-JP,es-ES}.json` 8 语言均新增 `logs.upstreamStatus`（如不复用 connFailed，再加 `logs.notCaptured`），译文语义一致。
- [ ] AC5：无 TS 编译错误（`yarn build` 通过）、无控制台 runtime 报错。
- [ ] AC6：旧数据（`upstream_status_code = 0` 或缺失、`attempts` 空数组）不崩、显示兜底占位。

# 影响范围（文件清单）

| 文件 | 改动 |
| --- | --- |
| `src/pages/Logs.tsx` | Meta grid 插入 upstream chip（行 315 附近）；attempts 条件 `> 1` → `>= 1`（行 325）|
| `src/locales/zh-CN.json` | 新增 `logs.upstreamStatus` / 可选 `logs.notCaptured` |
| `src/locales/en-US.json` | 同上 |
| `src/locales/ar-SA.json` | 同上 |
| `src/locales/fr-FR.json` | 同上 |
| `src/locales/de-DE.json` | 同上 |
| `src/locales/ru-RU.json` | 同上 |
| `src/locales/ja-JP.json` | 同上 |
| `src/locales/es-ES.json` | 同上 |

后端、DB schema、列表 API、ProxyLogDetail 类型 / RequestTabs 不动。

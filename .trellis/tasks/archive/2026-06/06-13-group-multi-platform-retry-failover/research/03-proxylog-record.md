# Research: proxy_log 记录结构现状

- **Query**: proxy_log schema(platform_id 单值能否记多尝试?); upsert 时机; 前端 Logs 展示字段; 一请求是否=一条 log; 支持"每次尝试记录"需加什么?
- **Scope**: internal
- **Date**: 2026-06-13

## 结论速览

- **一个用户请求 = 一条 proxy_log 行**。`upsert_log` 在请求各阶段被反复调用，但都用**同一个 `log.id`（请求开始时生成的 uuid）** 做 `INSERT OR REPLACE`，即"渐进式更新同一行"，不是多行。
- **`platform_id` 是单值 `INTEGER`**，只能记最终/当前一个平台。**当前结构无法表达"多次尝试 / 每次选了哪个平台"**。
- 支持"每次尝试记录"需新增存储：推荐 **`proxy_log` 加 JSON 数组列 `attempts`**（最低侵入，复用现有单行 upsert 模型），或建 `proxy_log_attempt` 子表（platform_id/status/duration/error 每尝试一行）。两者取舍是给 main 的决策点。

## Findings

### proxy_log schema（001_init.sql + models.rs）

| File:Line | 说明 |
|---|---|
| `migrations/001_init.sql:68-95` | proxy_log 表定义。**PK `id TEXT`（无连字符 uuid = 请求 ID）**，`platform_id INTEGER NOT NULL DEFAULT 0`（**单值**），含 status_code/upstream_status_code/duration_ms/tokens 等 |
| `models.rs:657-715` | `ProxyLog` struct，含 `platform_id: u64`（670 行）。无 attempts/retry 相关字段 |
| `db.rs:101` | migration 008 `ALTER TABLE proxy_log ADD COLUMN est_cost` |
| `db.rs:105` | migration 010 `ALTER TABLE proxy_log ADD COLUMN is_stream` |

→ 加列的既有模式：在 `init_tables` 里追加 `ALTER TABLE proxy_log ADD COLUMN attempts TEXT NOT NULL DEFAULT '[]'`（仿 manual_budgets JSON 列写法 `models.rs:310-320`）。

### upsert 写入时机（proxy.rs + db.rs）

| File:Line | 说明 |
|---|---|
| `proxy.rs:337-376` | `upsert_log()`：按 log_settings 裁剪字段 + 算 est_cost，调 `db::upsert_proxy_log` |
| `proxy.rs:569/584/595/601/614/623/633` | 请求早期各失败/阶段点反复 upsert（同一 log） |
| `proxy.rs:601/614/701/...` | "Upsert #1/#2/#3" 渐进更新：group resolved → route resolved → 完成 |
| `db.rs:1040-1053` | `upsert_proxy_log`：**`INSERT OR REPLACE INTO proxy_log` by `log.id`** — 同 id 覆盖整行 |

关键（`db.rs:1044-1047`）：
```rust
conn.execute(
  &format!("INSERT OR REPLACE INTO proxy_log ({PROXY_LOG_COLUMNS}) VALUES (?1,...,?28)"),
  params![log.id, ..., log.platform_id as i64, ...],
)?;
```
→ 一个请求生命周期内 `log.id` 不变 → 始终一行。重试若想记每次尝试，**不能靠多次 upsert 同一行**（会互相覆盖），必须用数组列或子表。

### 前端 Logs 展示（Logs.tsx + api.ts）

| File:Line | 说明 |
|---|---|
| `api.ts:510-525` | `ProxyLogSummary`（列表项）：含 `platform_id`（单值）、status_code、model、actual_model 等 |
| `api.ts:527-556` | `ProxyLogDetail`（详情）：单 platform_id + 单 upstream_request/response 套字段 |
| `Logs.tsx:445` | 列表行平台列：`platformMap.get(log.platform_id) \|\| "-"`（id→name 映射，单值） |
| `Logs.tsx:454,456-458` | 列表显示 actual_model + status_code（绿/红） |
| `Logs.tsx:224-237` | 详情解析单套 request/upstream/response 字段 |
| `Logs.tsx:57` | 过滤：`f.platform_id = Number(filterPlatform)`（按单平台过滤） |

新需求"列表显示最终选中平台 + 重试次数"→ Summary 加 `attempt_count` + 最终 platform_id（已有）即可。"详情看每次选择"→ Detail 加 `attempts: Attempt[]`，前端渲染列表。

## Caveats / Not Found

- 当前**没有任何 retry/attempt 概念的字段或子表**。
- `INSERT OR REPLACE` 决定了"渐进更新单行"是现有架构基石；若改子表方案需新增独立写入路径（不走 upsert_log 单行覆盖）。
- 存储结构选型（JSON 数组列 vs 子表）是明确决策点 → 见 04-fix-points 与回报。

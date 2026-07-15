# proxy_log 落库足迹调研

## 1. 表结构 (proxy_log)

定义: `gateway/db/schema_early.rs:76-103` (CREATE) + 多次 ALTER (schema_early 008/010/014, schema_late 重命名 group_name→group_key Migration ~27)。

### 区分"来源/类型"的现成字段

**`source_protocol TEXT`** (NOT NULL DEFAULT '') 是当前唯一用于标记请求类型的列。取值见全仓 grep:

| 值 | 来源 | 落库点 |
|---|---|---|
| `anthropic` | 代理转发 (count_tokens/主路径) | `proxy/count_tokens.rs:59`, `proxy/handler.rs:288` (detect_source_protocol) |
| `claude_code` | 透传 claude code | `proxy/passthrough.rs:19` |
| `openai_responses` | OpenAI responses API | `proxy/responses.rs:34` |
| `http-connect` | HTTP CONNECT 隧道 | `proxy/log.rs:299` |
| `passthrough_unmatched` | 未匹配透传 | `proxy/passthrough.rs:405` |
| `test` | **平台测试** (model_test) | `commands_ai_tools/model_test.rs:157` (build_test_proxy_log) |
| `fetch-models` | **拉取模型列表** | `commands_platform/model_fetch.rs:~95` (make_log, source_protocol="fetch-models") |
| `quota` | **余额查询** | `gateway/quota/http.rs:174` (make_quota_log) |

**无独立 type/source/kind 列** — 全部复用 `source_protocol` 做约定串标记。`group_key` 也用哨兵串辅助: test=`[test]` (model_test.rs:153), quota=`[quota]` (http.rs:171), fetch-models=`[fetch-models]` (model_fetch.rs)。model_test 还把真实 `platform_id` 落库; quota/fetch-models 落 `platform_id=0` (http.rs:176, model_fetch.rs)。

### 其他相关列
- `status_code` / `upstream_status_code` / `duration_ms` / `input_tokens`/`output_tokens`/`est_cost`
- `blocked_by`/`blocked_reason` (中间件拦截审计, schema_early.rs:231)
- `request_url`: test=`/model-test/{pid}`, quota=`/quota`, fetch-models=`/fetch-models` (也辅助区分)
- `is_stream`, `attempts`, `retry_count`, 软删 `deleted_at`

## 2. 落库点 (upsert 入口)

### 主代理转发 (渐进式 upsert_log)
统一入口 `proxy/log.rs:16` `pub(crate) async fn upsert_log(state, log, settings)`，内部走 `ProxyLogColumns` diff (insert_proxy_log_columns / update_proxy_log_columns, proxy_log.rs:242/268)。
调用点 (全在 `gateway/proxy/`): handler.rs:206/233/267/275/290/322/334/390/398/508, forward.rs:112/149/234/380/465, finish.rs:112/353, count_tokens.rs:113/124/191/213/228。

### 非代理直接 upsert_proxy_log (无渐进式 diff)
- **model_test** (测试): `commands_ai_tools/model_test.rs:283/323/360/372` 直调 `db::upsert_proxy_log` (INSERT OR REPLACE)
- **fetch_models** (拉模型): `commands_platform/model_fetch.rs` (成功/失败各一处) 直调 `db::upsert_proxy_log`
- **quota** (余额): `gateway/quota/http.rs:206 persist_quota_log` → `db::upsert_proxy_log`

### 入库 API 汇总 (gateway/db/proxy_log.rs)
- `upsert_proxy_log` (INSERT OR REPLACE 全列) — line 50, 供非渐进式 (test/quota/fetch-models) 用
- `insert_proxy_log_columns` (渐进式首节点 INSERT) — line 242
- `update_proxy_log_columns` (渐进式 diff UPDATE) — line 268

## 3. 现状: 谁落库谁不落

| 操作 | 落 proxy_log? | source_protocol | platform_id |
|---|---|---|---|
| 代理转发 (用户业务请求) | 是 (渐进式) | anthropic/claude_code/... | 真实 |
| 平台测试 model_test | 是 | `test` | 真实 |
| 拉取模型 fetch_models | 是 | `fetch-models` | 0 |
| 余额查询 platform_query_quota | 是 | `quota` | 真实 (经 task_local QUOTA_PLATFORM_ID scope) |
| NewAPI 余额 platform_query_quota_newapi | 是 | `quota` | 真实 |
| **cli_proxy_test (新, cpa-standalone)** | **否 (不落库)** | - | - |

## 4. cli_proxy_test (commands_cli_proxy/test_cmd.rs)

- line 16-32: `cli_proxy_test(db, id)` → 读 `cli_proxy_provider` 表 → 调 `gateway::quota::query_quota(Some db, &provider.base_url, &provider.api_key, 0)`。
- **platform_id=0 显式传入** (line 29)，注释 "persist_quota_to_db 的 None-guard 等价直接绕过"。
- 但 query_quota 内部仍调 `quota_get_json` → `persist_quota_log` 会落 proxy_log (source_protocol="quota", platform_id=0 via task_local try_get unwrap_or 0)。
- **关键发现**: cli_proxy_test 注释说"不落库"指的是不写 platform 表 (不 calibrate_from_quota)，但 **proxy_log 仍会被 query_quota 路径落一条** (platform_id=0)。注释 `platform_id=0：persist_quota_to_db 的 None-guard` 描述的是 platform 表余额回写，与 proxy_log 落库是两码事 — cli_proxy_test 实际已在 proxy_log 留痕 (quota 行, platform_id=0)。

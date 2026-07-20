# 平台测试 / 余额查询 / cli-proxy 测试 现状 + 前端 Logs 页

## 1. 平台测试 (model_test)

- Command: `model_test` (`commands_ai_tools/model_test.rs:264`)
- 请求路径: 直发 HTTP (reqwest, 不经过 proxy handler) — line 294-309 build client + post。复用 `gateway::http_client` + `gateway::proxy::apply_client_headers`，但不走 axum 路由/路由层。
- **落 proxy_log: 是**。每个分支 (mock/请求失败/非成功/成功) 都调 `db::upsert_proxy_log(build_test_proxy_log(...))` — line 283/323/360/372。
- 标记: `source_protocol="test"`, `group_key="[test]"`, `request_url="/model-test/{pid}"`, `platform_id`=真实。
- 前端消费: `get_last_test_result` (`gateway/db/usage_stats.rs:118`) 查 `WHERE platform_id=? AND source_protocol='test' ORDER BY created_at DESC LIMIT 1`，供 PlatformCard 徽章展示 ok/fail。前端 api: `platforms.ts:330 invoke("get_last_test_result")`。
- model_test mock 也落 log (model_test.rs:283)。

## 2. 平台余额查询 (platform_query_quota)

- Command: `platform_query_quota` (`commands_platform/quota.rs:10`) + `platform_query_quota_newapi` (line 26)
- 请求路径: 调 `gateway::quota::query_quota` / `query_quota_newapi` (quota/mod.rs:43) → 按 base_url 路由到 balance/coding_plan/newapi 子模块 → 统一经 `quota_get_json` (quota/http.rs:117) 出站 GET。
- **落 proxy_log: 是**。`quota_get_json` 单点落库 (http.rs:135/143/150/156)，source_protocol="quota", group_key="[quota]", request_url="/quota"。
- platform_id 经 task_local `QUOTA_PLATFORM_ID` (http.rs:12) 在 query_quota 入口 scope 设定 (mod.rs:44)，`make_quota_log` line 176 读取 → 真实 platform_id 落库。
- **额外**: 成功后 `persist_quota_to_db` (quota.rs:45) 调 `estimate::calibrate_from_quota` 写 platform 表余额字段 (est_balance_remaining 等) — 这是 platform 表回写，不是 proxy_log。

## 3. cli-proxy provider 测试 (cli_proxy_test)

- Command: `cli_proxy_test` (`commands_cli_proxy/test_cmd.rs:16`)
- 请求路径: 读 `cli_proxy_provider` → `gateway::quota::query_quota(db, base_url, api_key, 0)` (line 29) → 同 §2 quota 链路 → `quota_get_json` 出站。
- **落 proxy_log: 是 (间接)**。query_quota 必经 quota_get_json 落库。但 platform_id=0 (test_cmd.rs:29 显式传 0)，task_local scope=0 → make_quota_log platform_id=0。
- **注释"不落库"的真相**: test_cmd.rs:4/11/28 注释说不落库，指的是 **不调 persist_quota_to_db → 不写 platform 表** (provider 不在 platform 表，无 platform_id 可回写)。proxy_log 的 quota 行仍会落 (platform_id=0, 无法归属到具体 provider)。
- **缺口**: cli_proxy_test 的 proxy_log 行 platform_id=0，**无法从 proxy_log 反查是哪个 provider**。provider id 没有任何字段承载 (group_key="[quota]" 也没带)。这是用户需求①"独立请求日志"的核心痛点 — cli-proxy 测试请求在 Logs 里混在 platform_id=0 的 quota 行中，无法区分归属。

## 4. 前端 Logs 页 (src/pages/Logs/)

- 入口 `Logs.tsx` 拆 4 子: useLogsData / ListView / DetailPanel / primitives。
- 筛选维度 (useLogsData.ts:44-67): platform / group / status(success/error) / time preset / model(original|actual) / path(LIKE request_url)。**无 source_protocol 筛选**。
- 列表查询 `proxyLogApi.list` / `filtered_list_proxy_logs` (后端 proxy_log.rs:340) → SQL `WHERE deleted_at=0` + filter。**test/quota/fetch-models 行与业务请求混在一起显示**，除非用户用 path 筛 "/quota" 或 platform 筛排除。
- Summary 行带 source_protocol 字段 (models/proxy_log.rs:124 ProxyLogSummary 有 source_protocol)，但 ListView.tsx 未用它做筛选 UI。
- api 封装: `services/api/` proxyLogApi (list/listFiltered/count/get/clear)。types: ProxyLogSummary/ProxyLogDetail/ProxyLogFilter (part1.ts/part2.ts)。ProxyLogFilter 无 source 字段。
- 详情 DetailPanel 展示 source_protocol (useLogsData.ts copyDetail 里 `Source Protocol: ${d.source_protocol}`)。

## 5. 设计启示 (交 main + 用户裁)

### 用户需求映射
- ① cli-proxy provider 测试要有独立请求日志 → 当前 cli_proxy_test 经 query_quota 已落 proxy_log(quota 行)，但 platform_id=0 无法归属 provider。
- ② AI 平台余额查询/测试迁入此请求日志 → 余额(query_quota)和测试(model_test)本就已落 proxy_log (source_protocol=quota/test)。

### 选项 (不拍板)

**A. 复用 proxy_log，加 source 维度 (最小 diff，YAGNI 优先)**
- 现有 source_protocol 约定串 (test/quota/fetch-models) 已天然是"请求类型"维度。
- cli_proxy_test 需让 proxy_log 行能归属 provider: 方案 — 给 query_quota 增加"外部表注入 provider_id"机制 (类似 candidates.rs read_cli_proxy_provider_id)，或 cli_proxy_test 落库时 platform_id 填一个映射值。但 cli_proxy_provider 独立表，platform_id 列指向 platform 表，语义不合。
- 前端 Logs 加 source_protocol 筛选 (ProxyLogFilter 加 source 字段 + ListView 加下拉)，把 test/quota/fetch-models 业务请求分流展示。
- 优势: 零新表，复用现有 upsert/retention/聚合/Logs UI 基建。劣势: platform_id 语义对 cli_proxy_provider 不适配 (provider id ≠ platform id)；quota 行 platform_id=0 混杂。

**B. 新建独立 request_log 表 (cli_proxy_provider 专用)**
- 单独表记录 cli-proxy provider 测试请求 (provider_id 外键 + url/status/body/duration)。
- 优势: provider 归属干净，不污染 proxy_log。劣势: 新表 + 新 CRUD + 新前端页/Tab，重复 retention/展示逻辑；与"余额/测试迁入"需求②语义割裂 (余额仍在 proxy_log)。

**C. 混合: proxy_log 加 cli_proxy_provider_id 列 (可空外键)**
- proxy_log 增 `cli_proxy_provider_id INTEGER DEFAULT 0` 列。cli_proxy_test 落库时填 provider id (需让 query_quota 透传 provider_id 到 make_quota_log，或在 test_cmd 包一层自落库)。
- 前端 Logs 筛选 + CliProxy 页独立视图查同一表 WHERE cli_proxy_provider_id>0。
- 优势: 单表统一请求日志，provider 归属可查，复用基建。劣势: 加列 migration + quota 路径需透传 provider_id (task_local 扩展或新参)。

### 关键约束
1. **query_quota 是共享入口** (mod.rs:43)，cli_proxy_test / platform_query_quota / cold_start_init_tray_estimates 都走它。任何"落库归属"改动在此函数链扩散影响面 (高频路径最小 diff 硬规)。
2. **platform_id=0 是 None-guard 约定** (test_cmd.rs:28 注释)，多个调用点依赖。若复用 platform_id 表达 provider 归属会破坏约定。
3. **model_test 已落 test 行 + get_last_test_result 消费** — 迁移/复用需保持徽章查询不断链。
4. **quota_get_json 是 quota 唯一出站落库点** (http.rs:117)，所有 quota 子模块经此 — 改落库策略在此单点生效，不需改 10 个 provider 函数。
5. **source_protocol 约定串无 enum 约束** — 纯字符串，加新值零 schema 成本但缺类型安全。
6. 前端 Logs 已有 source_protocol 字段在手 (Summary + Detail)，加筛选是纯前端 + filter struct 加字段的小改 (build_filter_where 加分支)。

### 需要 (交 main 转达用户)
- "独立请求日志"是指: (a) Logs 页里能筛出 cli-proxy 测试记录即可 (复用 proxy_log + 筛选)，还是 (b) CliProxy 页内嵌专属日志视图，还是 (c) 完全独立的新表/新页？这决定 A/B/C 走向。
- 余额/测试"迁入"是指: Logs 页统展示这三类 (加 source 筛选)，还是把它们从 Logs 主列表移出到独立 Tab？

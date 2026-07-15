# 设计 — cli-proxy 测试 + 平台余额/测试 统一请求日志

## 架构

**复用 proxy_log 表, 不新建表**(research §7 选项 C)。三类管理操作(test/quota)本就落 proxy_log, 只需: ① 加 provider 归属列 ② 新前端页按 source 分流 ③ Logs 主页排除。

```
数据层 (复用)
  proxy_log 表
    + cli_proxy_provider_id INTEGER NULL  (新列, migration)
    现有 source_protocol TEXT 区分类型 (test/quota/anthropic/...)

落库链路 (透传 provider_id)
  cli_proxy_test ──┐
                   ├─→ query_quota(task_local QUOTA_PLATFORM_ID=0
                   │                  + QUOTA_CLI_PROXY_PROVIDER_ID=<pid>)
                   │   → quota_get_json → upsert_proxy_log(provider_id 落库)
  platform_query_quota ─→ query_quota(provider_id=NULL)  (不变)
  model_test ──────────→ upsert_proxy_log(provider_id=NULL)  (不变, 加空槽)

后端 command
  list_request_logs(新)   ← source IN(test/quota) + provider_id 筛选 + 分页
  list_proxy_logs(改)     ← 加 exclude_sources 参数, Logs 主页传 test/quota

前端
  侧栏: Logs(代理转发) + 请求日志(新页, test/quota)
  RequestLog.tsx(新) ── list + 筛选 + 详情(复用 Logs primitives)
  Logs 主页 ──────── 默认排除 test/quota
```

## 数据流

### cli-proxy provider 测试(provider_id 透传)
1. 前端 CliProxy 页点「测试」→ invoke `cli_proxy_test(provider_id)`
2. `commands_cli_proxy/test_cmd.rs` → `gateway::quota::query_quota(...)`
3. query_quota 设 task_local `QUOTA_CLI_PROXY_PROVIDER_ID = provider_id`(与 QUOTA_PLATFORM_ID=0 并存)
4. quota_get_json 落库时读 task_local, `upsert_proxy_log` 传 `cli_proxy_provider_id = Some(pid)`
5. 新页按 provider_id 筛选该 provider 测试历史

### 平台余额/测试(不变)
- platform_query_quota → query_quota(provider_id=NULL) → 落库 provider_id NULL
- model_test → upsert_proxy_log(provider_id=NULL)
- get_last_test_result 查 source_protocol='test' 最新 → 徽章正常(同表分流不影响后端查询)

### 前端分流
- Logs 主页: invoke list_proxy_logs(exclude_sources=['test','quota']) → 纯代理转发
- 请求日志页: invoke list_request_logs(sources=['test','quota'], provider_id?, ...) → 管理操作

## 取舍

| 决策 | 选 | 理由 |
|---|---|---|
| 表 | 复用 proxy_log | 独立表重复 retention/CRUD/详情基建; provider_id 列足够归属 |
| Logs 主页 test/quota | 迁出(排除) | 用户「迁入请求日志」= Logs 纯代理转发, 新页专放管理操作 |
| fetch-models/cold_start | 不动(留 Logs) | 用户只提测试/余额; fetch-models 量小不显眼; 后续可迭代 |
| provider_id 列 | 可空 INTEGER | 转发行/平台行 NULL, cli-proxy 测试行有值; 兼容存量 |
| 透传机制 | task_local(同 QUOTA_PLATFORM_ID) | query_quota 是共享高频入口, task_local 已是 idiom, 最小 diff |
| 前端组件 | 复用 Logs primitives(ListView/DetailPanel) | 独立筛选 state + 独立页, 但行渲染/详情面板复用, 避重复 |

## 关键约束 / 不变量

- **quota_get_json 单点** — provider_id 透传必须在此挂, 禁在多处落库
- **QUOTA_PLATFORM_ID=0 不复用表达 provider** — 0 是 None-guard 约定, provider 走独立 task_local 槽
- **get_last_test_result 不断链** — 后端查 source_protocol='test' 最新一条, 前端 Logs 分流不影响
- **migration 幂等** — ALTER TABLE ADD COLUMN 吞重复(SQLite 不报错或 IF NOT EXISTS idiom)
- **高频路径最小 diff** — query_quota 是 cli_proxy_test/platform_query_quota/cold_start 共享, 只加 task_local 读, 不改签名波及 N 调用点(参考 memory [[high-freq-path-min-diff]])
- **upsert_log/upsert_proxy_log 对称** — 两落库路径都需支持 provider_id 列(对称 cap, 参考 [[symmetric-body-cap]])

## 技术选型

- migration: schema_late.rs 新 ALTER(编号 047, 接 cpa-standalone 046 后)
- task_local: `thread_local! { static QUOTA_CLI_PROXY_PROVIDER_ID: Cell<i64> }`(同 QUOTA_PLATFORM_ID idiom)
- ProxyLogColumns: 加 `cli_proxy_provider_id: Option<i64>` 字段(diff 落库)
- upsert_proxy_log: INSERT OR REPLACE 加列
- 前端 api: `requestLogApi.list({ sources, providerId?, platformId?, status?, ... })`
- 侧栏: BASE_NAV 加 `{ id: "request-log", icon: "log", section: "nav.section.proxy" }`(Logs 同段)

## subtask 拆分(初拟, 落 task.json)
- s1 migration: proxy_log 加列(schema_late 047)
- s2 quota 链透传 provider_id(task_local + quota_get_json 落库 + ProxyLogColumns/upsert 对称) — 依赖 s1
- s3 后端 command: list_request_logs(新) + list_proxy_logs 加 exclude_sources — 依赖 s2
- s4 前端 RequestLog 页 + 侧栏 + api + i18n — 依赖 s3
- s5 前端 Logs 主页排除 test/quota + 筛选复用 — 依赖 s3(与 s4 并行, 不同文件)

依赖 DAG: s1→s2→s3→{s4,s5}

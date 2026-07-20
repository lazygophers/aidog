# cli-proxy 测试 + 平台余额/测试 统一请求日志 — PRD (主入口)

## 目标

新建独立「请求日志」侧栏页, 汇聚三类**非代理转发**的管理操作日志: 平台测试、平台余额查询、cli-proxy provider 测试。复用 `proxy_log` 表(不新建表), 加 `cli_proxy_provider_id` 可空列让 cli-proxy 测试归属到具体 provider。Logs 主页迁出 test/quota 两类变纯代理转发视图。

用户价值: 当前这三类操作本就落 proxy_log(source_protocol='test'/'quota'), 但 Logs 页无类型筛选跟代理转发业务请求混显, 用户看不到测试/余额历史; cli-proxy 测试 platform_id=0 无法归属 provider。独立页 + 类型筛选 + provider 归属后, 测试/余额日志可独立回溯, Logs 主列表干净。

成功长什么样:
- 侧栏新菜单「请求日志」(proxy section, 与 Logs/platforms 同段), 进去是 test/quota 两类日志列表(按时间倒序)
- 列表筛选: 类型(全部/测试/余额)/ 平台 / cli-proxy provider / 状态 / 时间
- 详情面板: 单条日志的请求/响应(token/cost/url/status/model + provider 归属)
- cli-proxy provider 测试行带 provider_id 归属, 能按 provider 筛测试历史
- Logs 主页只显示代理转发(test/quota 迁出), 不再混显管理操作
- get_last_test_result 徽章链不断(后端仍查 source_protocol='test' 最新一条, 不受前端分流影响)

- [x] 目标已定

## 边界

**范围内**:
1. Rust migration: `proxy_log` 加 `cli_proxy_provider_id INTEGER NULL` 列(schema_late 幂等 ALTER)
2. Rust quota 链透传: `query_quota` task_local 加 provider_id 槽, cli_proxy_test 传入, `quota_get_json` 落库带 provider_id; `ProxyLogColumns`/`upsert_log`/`upsert_proxy_log` 支持新列
3. Rust 后端 command: 新 `list_request_logs`(过滤 source_protocol IN test/quota + provider 筛选 + 分页); `list_proxy_logs`/`filtered_list_proxy_logs` 加排除 test/quota 选项(Logs 主页纯代理)
4. 前端新页 `RequestLog.tsx` + 侧栏菜单(proxy section) + api 封装 + i18n 8 locale
5. 前端 Logs 主页: 默认排除 test/quota(迁出到新页)
6. 筛选维度复用 Logs primitives(ListView/DetailPanel 局部复用, 独立筛选 state)

**范围外(非目标)**:
- 不新建独立日志表(复用 proxy_log, 避重复 retention/CRUD/展示基建)
- 不动 fetch-models / cold_start source(留 Logs 主页或后续迭代; 本次只迁 test/quota)
- 不改 proxy_log retention 逻辑(provider_id 列随主行生命周期)
- 不改代理转发落库路径(只加列, 转发行 provider_id=NULL)
- 不动 group/quota/pricing 核心逻辑

**已知约束**:
- `quota_get_json` (`gateway/quota/http.rs:117`) 是 quota 唯一出站落库单点, provider_id 透传在此挂
- `QUOTA_PLATFORM_ID` task_local (`http.rs:12`) 已存 platform_id, 加 `QUOTA_CLI_PROXY_PROVIDER_ID` 同 idiom
- `get_last_test_result` (`usage_stats.rs:118`) 查 `source_protocol='test'` 最新一条喂 PlatformCard 徽章 — 不断链
- `model_test` (`commands_ai_tools/model_test.rs:264`) reqwest 直发不经 proxy handler, 走 upsert_proxy_log 落库(已落, provider_id 透传加在此)
- `cli_proxy_test` (`commands_cli_proxy/test_cmd.rs:16`) 经 query_quota(platform_id=0), provider_id 传入后落库可归属
- proxy_log `source_protocol` 约定串区分: anthropic/claude_code(代理转发) vs test/quota/fetch-models/http-connect(管理/探测)

- [x] 边界已定

## 验收标准
- [ ] proxy_log 加 `cli_proxy_provider_id` 列 migration 幂等
- [ ] cli_proxy_test 落库带真实 provider_id(非 0), 能按 provider 筛
- [ ] 平台余额查询/测试落库不受影响(provider_id NULL, 徽章链不断)
- [ ] 新后端 command list_request_logs: source IN(test/quota) + provider 筛选 + 分页
- [ ] Logs 主页 list 排除 test/quota(纯代理转发)
- [ ] 侧栏新菜单「请求日志」(proxy section), 8 locale i18n 补全
- [ ] RequestLog 页: list + 筛选(类型/平台/provider/状态/时间) + 详情面板
- [ ] get_last_test_result 徽章仍正常(查 test 最新一条)
- [ ] cargo clippy --workspace 无新增; cargo test --workspace 过
- [ ] yarn build 过; check:i18n 全绿

## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [research/](research/) (proxy-log-footprint.md / test-balance-current.md)
- 任务/子任务/调度: task.json (`skein.py subtask list cli-proxy-request-log`)

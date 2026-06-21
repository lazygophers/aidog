export const meta = {
  name: 'stats-agg-table',
  description: '新增 stats_agg_hourly 聚合统计表 (小时×模型×分组×平台)，写入解耦日志开关，替换现有统计读取源，retention 默认 1 年可配',
  phases: [
    { title: 'Backend' },
    { title: 'Frontend' },
    { title: 'Verify' },
    { title: 'Fix' },
  ],
}

// ───────────────────────────────────────────────────────────
// 共享背景 (调研已确认的 file:line 事实，喂给每个 fresh-context agent)
// ───────────────────────────────────────────────────────────
const FACTS = `
## 已确认现状 (调研结论，file:line 为准)
### 写入路径
- proxy.rs \`upsert_log()\` (~line 477)，第二行 \`if !settings.enabled { return; }\` (~478) 早退 —— ⚠️ 关键：现有 proxy_log 写入受日志开关 gate。
- est_cost 计算 ~487-506；is_terminal 判定 ~513 (\`status_code != 0 && response_body != "[stream]"\`)；终态 emit "proxy-log-updated"/"tray-refresh" ~539-552。
- ⚠️ 因为 !enabled 时 upsert_log 在 478 直接 return，连 est_cost 都不算。要做到「不开日志也有统计」，必须把【聚合写入 + 所需的 token/cost/status 计算】抽到一条**不受 settings.enabled 影响**的独立路径，在请求终态无条件执行。不能只在 upsert_log gate 之后加。
### proxy_log 可用列 (回填/聚合源)
- created_at(UTC ms), model, actual_model(优先), group_key(UNIQUE, 无连字符), platform_id(0=auto 需回溯), status_code, duration_ms, input_tokens, output_tokens, cache_tokens, est_cost, deleted_at(=0 有效)。
- 本地时分桶: \`strftime('%Y-%m-%d %H:00:00', created_at/1000, 'unixepoch', 'localtime')\`。
- auto 平台回溯: platform_id=0 时取 \`(SELECT CAST(g.auto_from_platform AS INTEGER) FROM "group" g WHERE g.group_key=proxy_log.group_key AND g.auto_from_platform!='' AND g.deleted_at=0 LIMIT 1)\`，否则用 platform_id，记为 eff_pid。
### 读取点 (要切到 agg 表)
- 后端 db.rs: today_stats(~1497), today_platform_stats(~1570), get_group_usage_stats(~3375), get_all_group_usage_stats(~3390), query_stats_inner(~3714)。
- 命令 lib.rs: tray_today_stats(~276), popover_platform_today(~362), stats_query(~809), stats_query_batch(~821), group_usage_stats(~1535), all_group_usage_stats(~1542)。
- 前端消费: Home.tsx, Stats.tsx, Groups.tsx, PopoverConfigTab.tsx (api: statsApi/trayConfigApi.todayStats/popoverConfigApi.platformToday/groupUsageApi)。
### migration
- migrations/ 现存 001-010；下一个 = \`011_stats_agg_hourly.sql\`。注册在 db.rs init_tables() 的 include_str! 链 (~209-211 区域，与 001/002/003 同款 execute_batch)。
### settings 端到端样例 (照搬 retention_days)
- 模型: models.rs ProxyLogSettings(~1219), retention_days default 90。
- 命令: lib.rs proxy_log_settings_get/set(~1563-1605)；invoke_handler 注册(~4370)。
- 前端: api.ts(~1358) 类型+invoke 封装；AppSettings.tsx 对应 tab 渲染。
### cleanup
- db.rs cleanup_proxy_logs(~3065) + retention_cutoff(days)->Option<i64>(0=永久)。触发: proxy_log_settings_set 末尾 + 启动。
### 决策 (用户已定)
- 失败 = 非 2xx (status_code 不在 200-299)。success_count=2xx，error_count=终态非2xx。
- 回填 = 一次性 migration 把存量 proxy_log 聚合进 agg 表 + 提供手动「从 proxy_log 重建聚合表」命令(stats_rebuild_from_logs)。
- retention 默认 365 天 (stats_retention_days)，可配，0=永久。
- 分钟/5min 粒度 agg 表不覆盖 → Stats 这两档仍查 proxy_log；hourly 稀疏自动降级改为降到 proxy_log minute (不是 agg)。
- ⚠️ db::TodayStats 近期已加 input_tokens/output_tokens/cache_tokens 字段 —— agg 版 today_stats 必须填这三个。
## 项目门禁
- Rust: \`cd src-tauri && cargo clippy -- -D warnings\` (= make lint) 必须零 warning；\`cargo test\` 不回归。
- 前端: \`yarn build\`；\`node scripts/check-i18n.mjs\` 零缺失。
- i18n 8 locale (src/locales/*.json) 新 key 全补。
- 项目授权 git commit (禁 push)，格式 conventional commits。
`

const REVIEW_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    findings: {
      type: 'array',
      items: {
        type: 'object',
        additionalProperties: false,
        properties: {
          severity: { type: 'string', enum: ['critical', 'major', 'minor'] },
          file: { type: 'string' },
          line: { type: 'string' },
          issue: { type: 'string' },
          fix: { type: 'string' },
        },
        required: ['severity', 'file', 'issue', 'fix'],
      },
    },
    summary: { type: 'string' },
  },
  required: ['findings', 'summary'],
}

// ───────────────────────────────────────────────────────────
phase('Backend')
const backend = await agent(`你是 aidog Rust 后端工程师。实现 stats_agg_hourly 聚合统计表的**全部后端 + 数据契约**。

${FACTS}

## 目标
1. migration \`migrations/011_stats_agg_hourly.sql\`: 建表 stats_agg_hourly(id PK AUTOINCREMENT, time_hour TEXT 本地时, model TEXT, group_key TEXT, platform_id INTEGER /*存 eff_pid 回溯后*/, request_count, success_count, error_count, sum_input_tokens, sum_output_tokens, sum_cache_tokens, sum_est_cost REAL, sum_duration_ms INTEGER /*用 SUM 不用 AVG，便于再聚合；avg 在查询时算*/, created_at, updated_at, deleted_at, UNIQUE(time_hour,model,group_key,platform_id)) + 时间/模型/分组/平台 索引。在 db.rs init_tables() include_str! 链注册。
2. \`db::upsert_stats_agg(&db, cols)\`: 对一条终态请求，按 (time_hour,model,group_key,eff_pid) UPSERT (\`INSERT ... ON CONFLICT(...) DO UPDATE SET request_count=request_count+1, success_count=...+(2xx?1:0), error_count=...+(非2xx?1:0), sum_*=sum_*+?, updated_at=?\`)。model 取 actual_model 非空否则 model；eff_pid 按 auto 回溯规则算 (可在 SQL 内 CASE 或 Rust 预算后传入)。
3. **写入解耦**: 重构 proxy 请求终态路径，使 upsert_stats_agg 在请求终态**无条件调用 (不受 ProxyLogSettings.enabled 影响)**。注意 upsert_log 在 !enabled 时早退且不算 est_cost —— 你需要保证 agg 路径自己拿到 input/output/cache tokens + est_cost(必要时调 calc_est_cost) + status_code。避免与 proxy_log 写入重复计算时尽量复用。失败非致命 (tracing::warn 不中断请求)。
4. **读取源切换**: 把 today_stats / today_platform_stats / get_group_usage_stats / get_all_group_usage_stats 改为从 agg 表查 (today_stats 须填 input/output/cache 三字段)。query_stats_inner: hourly/daily 粒度 + 任意 filter(group/model/platform)/group_by 从 agg 表查；minute/5min 粒度仍查 proxy_log (保留原逻辑分支)。平台维度查询记得 platform 名 JOIN。
5. **回填**: migration 011 内 (或 init 时一次性、幂等) 把存量 proxy_log 按上述分桶规则聚合 INSERT 进 agg 表 (注意 localtime + eff_pid 回溯 + 2xx 判定 + deleted_at=0)。幂等: 用 INSERT OR IGNORE 或先判空表再回填，避免重复执行翻倍。
6. **手动重建命令** \`stats_rebuild_from_logs\`: 清空 agg 表后从 proxy_log 全量重建 (用户启用 log 后修复用)。
7. **settings**: models.rs 加 StatsSettings{ retention_days: u32 (default 365) }；lib.rs 加 stats_settings_get/set 命令 (参考 proxy_log_settings_*)，set 末尾触发 cleanup_stats_agg；invoke_handler 注册 get/set + stats_rebuild_from_logs。
8. **cleanup**: db.rs 加 cleanup_stats_agg(&db, retention_days) (参考 cleanup_proxy_logs, 复用 retention_cutoff, 0=永久)；在 stats_settings_set 末尾 + 启动流程调用。
9. **api.ts 数据契约**: 在 src/services/api.ts 加 StatsSettings 类型 + statsSettingsApi(get/set) + statsApi 里 rebuildFromLogs 封装。**只动 api.ts 的类型与 invoke 封装 (前端契约层)，不动任何 .tsx 页面 (页面由下游 Frontend 阶段改)。** 若 today_stats/platform/group 返回结构有新增字段，同步 TS 类型。

## 验收
- \`cd src-tauri && cargo clippy -- -D warnings\` 零 warning；\`cargo test\` 不回归 (新增 agg upsert/回填/2xx判定 单测优先)。
- \`yarn build\` 通过 (api.ts 类型自洽)。
- 自查: agg 写入不受日志开关影响 (关日志也写)；回填幂等不翻倍；today_stats 填了 input/output/cache。

## 失败处理
- 列名/函数签名不确定 → Read 确认再改，不臆测。clippy/test 红修到绿。
- 完成后 git commit (\`feat(stats): stats_agg_hourly 聚合表 + 写入解耦 + 读取源切换 + 回填\`)，禁 push。.git/index.lock 冲突等 3 秒重试 ≤3 次。

返回: 改了哪些文件、新增命令/函数清单、migration 内容要点、agg 写入挂载点 file:line、cargo clippy+test+yarn build 实际输出摘要。`, { phase: 'Backend' })

// ───────────────────────────────────────────────────────────
phase('Frontend')
const frontend = await agent(`你是 aidog React 前端工程师。后端 stats_agg_hourly + 数据契约已落地，现在切换前端读取 + 加设置 UI。

${FACTS}

## 后端已交付摘要
${backend}

## 目标
1. 确认 Stats.tsx / Home.tsx / Groups.tsx / PopoverConfigTab.tsx 的统计读取经 api.ts 封装 —— 若后端保持了 API 签名/返回结构兼容，这些页**多数无需改**，只需验证渲染正常。逐页核对。
2. Stats.tsx: hourly/daily 粒度走 agg (透明)；minute/5min 仍走 proxy_log。**自动降级逻辑** (现有 hourly 稀疏→降 5min) 改为 hourly 稀疏→降 proxy_log minute 查询。若 minute/5min 选了超出 proxy_log 保留范围，给诚实提示「该粒度数据仅短期可用」。
3. AppSettings.tsx: 在合适 tab (日志/统计相关) 加「聚合统计保留天数」输入 (stats_retention_days，0=永久)，走 statsSettingsApi。加「从日志重建统计」按钮 (statsApi.rebuildFromLogs)，带 loading + 完成提示。
4. i18n: 新文案 t("stats.xxx"/"settings.xxx","中文兜底") 全部同步 src/locales/ 8 个 locale。
5. 反 slop: 沿用 Liquid Glass + 现有组件，不加装饰。

## 验收
- \`yarn build\` 通过；\`node scripts/check-i18n.mjs\` 零缺失。
- 自查: 所有统计读取点正常渲染；minute/5min 降级到 proxy_log；retention 设置 + 重建按钮可用。

## 失败处理
- api 签名与预期不符 → Read api.ts 后端交付确认。build/check-i18n 红修到绿。
- 完成后 git commit (\`feat(stats): 前端切聚合表读取 + retention 设置 + 重建按钮\`)，禁 push。index.lock 冲突等 3 秒重试 ≤3 次。

返回: 改了哪些文件、新增 i18n key、yarn build + check-i18n 实际输出。`, { phase: 'Frontend' })

// ───────────────────────────────────────────────────────────
phase('Verify')
const LENSES = [
  { key: 'sql-backfill', prompt: 'SQL 与回填正确性: 审 011 migration 建表+索引+UNIQUE 键、upsert ON CONFLICT 累加逻辑、回填 SQL 的本地时分桶(localtime)、eff_pid auto 回溯 CAST、2xx 判定、deleted_at 过滤、幂等(不重复翻倍)。重点找会导致统计数字错的 bug。' },
  { key: 'decouple', prompt: '写入解耦正确性: 确认 upsert_stats_agg 在请求终态【关闭日志时也执行】(不被 ProxyLogSettings.enabled gate)，且拿到正确的 input/output/cache/est_cost/status_code；确认重试/流式场景不会重复计数 (一次请求只聚合一次终态)；失败非致命。' },
  { key: 'parity', prompt: '读取源切换 parity: 对 today_stats/today_platform_stats/group_usage/query_stats，逐个核对 agg 表查询结果与原 proxy_log 查询在相同输入下是否一致 (字段齐全、含 today input/output/cache 三字段、平台名 JOIN、group_by 维度、排序)。minute/5min 是否正确仍走 proxy_log。' },
  { key: 'settings-cleanup', prompt: 'settings + cleanup + 命令: StatsSettings 端到端 (model/命令/注册/api.ts/UI) 是否串通；cleanup_stats_agg retention 0=永久、触发点齐全 (set+启动)；stats_rebuild_from_logs 清空+重建正确；invoke_handler 是否注册全。' },
  { key: 'frontend', prompt: '前端: 所有统计读取点 (Home/Stats/Groups/Popover) 渲染正确无 undefined；Stats 自动降级改为 proxy_log minute；retention UI + 重建按钮可用；i18n 8 locale 全覆盖无裸 key。' },
]
const reviews = await parallel(LENSES.map(l => () =>
  agent(`你是 aidog 资深审查员 (只读，不改码)。后端+前端已实现 stats_agg_hourly 功能并提交。针对以下维度审查，对抗性找真 bug：

${FACTS}

## 后端交付
${backend}
## 前端交付
${frontend}

## 审查维度: ${l.key}
${l.prompt}

用 Read/Grep 核实代码 (git diff 或直接读文件)。只报**确有依据**的问题，每条给 file:line + 具体修法。无问题返回空 findings。`,
    { label: `verify:${l.key}`, phase: 'Verify', schema: REVIEW_SCHEMA, agentType: 'Explore' })
))

const allFindings = reviews.filter(Boolean).flatMap(r => r.findings || [])
const blocking = allFindings.filter(f => f.severity === 'critical' || f.severity === 'major')
log(`Verify: ${allFindings.length} findings (${blocking.length} blocking)`)

// ───────────────────────────────────────────────────────────
phase('Fix')
if (blocking.length === 0) {
  log('无 blocking 问题，跳过 Fix 阶段')
  return { backend, frontend, findings: allFindings, fixed: false }
}
const fix = await agent(`你是 aidog 工程师。修复 stats_agg_hourly 功能审查发现的 blocking 问题。

${FACTS}

## 待修问题 (critical/major)
${JSON.stringify(blocking, null, 2)}

## 全部 findings (含 minor 参考)
${JSON.stringify(allFindings, null, 2)}

逐条核实并修复 (minor 顺手修)。修完跑全门禁: \`cd src-tauri && cargo clippy -- -D warnings\` + \`cargo test\` + \`yarn build\` + \`node scripts/check-i18n.mjs\`，全绿。git commit (\`fix(stats): 修复聚合表审查问题\`)，禁 push。

返回: 每条问题如何修的、门禁实际输出。`, { phase: 'Fix' })

return { backend, frontend, findings: allFindings, fix, fixed: true }

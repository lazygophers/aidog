- 本项目授权自动 `git commit`：所有文件变更完成后立即提交，无需等待明确指令
- 提交信息格式：`<type>(<scope>): <description>`，type 遵循 conventional commits（feat / fix / chore / style / refactor / docs）
- 禁 `git push`，等明确指令

## 技术栈

Tauri 2.0 + React 19 + TypeScript + Rust + Yarn

## 快速开始

```bash
yarn                          # 装前端依赖
yarn tauri dev                # 启动桌面应用（dev）
yarn build                    # 前端构建（tsc && vite build）
cd src-tauri && cargo build   # 仅构 Rust 后端
cd src-tauri && cargo clippy  # Rust lint（warning 必须清）
cd src-tauri && cargo test    # Rust 测试（db/proxy/converter/router/usage_color 等有 #[test]）
```
> 前端无 lint/test/format 脚本（package.json 仅 dev/build/preview/tauri）。

## 项目结构

```
src/                    # React 前端
  pages/                # 页面组件 11 个（Platforms/Groups/Logs/Settings/AppSettings/CodexSettings/ModelTestPanel/PopoverConfigTab/PricingTab/Stats/TrayConfigTab）— Settings 为编排容器，子组件见 components/settings/
  components/
    settings/           # 设置页拆分组件（editors.tsx 全部字段/特殊编辑器 + 令牌 F/S + Header/AnchorNav/UnsavedModal）
    shared/             # 三页共享展示组件（CompactCard/StatChip/BalanceBar/colorScale/usageColor）
  services/api.ts       # TS 类型定义 + Tauri invoke 封装
  themes/               # 每主题 light/dark CSS 变量
  utils/                # pinyin(拼音搜索) / formatters(统一数值格式化) / navGuard(无路由离页拦截)
src-tauri/src/
  lib.rs                # Tauri commands（73 个）
  gateway/
    adapter/            # 协议转换（15 provider adapter + converter + types）
      converter.rs      # convert_request / parse_sse / parse_incoming_request
    codex.rs            # Codex TOML 设置子系统（CodexSettings）
    db.rs               # SQLite CRUD + settings + usage stats + cleanup
    estimate.rs         # est_cost 估算（必走 resolve_price 回退链）
    http_client.rs      # 共享 reqwest client
    i18n.rs             # 后端 i18n
    manual_budget.rs    # 手动预算
    models.rs           # 所有 Rust 数据模型（Protocol 枚举 62 变体）
    price_sync.rs       # 价格同步
    proxy.rs            # Axum 代理服务器（渐进式日志、超时级联、log settings 感知、POST /api/group-info）
    quota.rs            # 平台余额 & Coding Plan 配额查询（DeepSeek/StepFun/SiliconFlow/OpenRouter/Novita + Kimi/GLM/MiniMax + NewAPI）
    router.rs           # 分组匹配 + 模型映射 + 平台选择
    usage_color.rs      # 金额颜色（按使用速率 pace/days_remaining）
```

## 关键约束

### 平台默认配置 (platform-presets.json)
- 真值源 = `src-tauri/defaults/platform-presets.json`（61 协议，含 `last_updated` Unix 秒；非 `resources/`）。手维护，禁机器生成覆盖。
- Rust 入口 `commands/defaults.rs::get_defaults_json`：读 app data (`~/.aidog/platform-presets.json`) → 缺失/损坏回退 `include_str!` bundled（同 `settings.json` 模式，非 Tauri resources）。
- 前端 4 函数（`src/domains/platforms/defaults.ts` 的 `getDefaultEndpoints` / `getDefaultModels` / `getDefaultModelList` / `defaultClientForProtocol`）**全部 async**；模块级 `docPromise` 单次 RPC 缓存（多次调用复用同一 IPC）。**所有 caller 必须 `await`**（或 `.then`）—— 改动 grep 调用点确认无遗漏，TS 编译会捕获漏 await。
- mock 协议不在 JSON 内（`platformPaste.ts:15` 从 `matchPlatform` 排除）；`getDefaultEndpoints("mock")` 返 `[]`。
- **coding_plan 分支与 glm_coding 独立协议**：preset JSON **默认不带** `coding_plan` 子分支（endpoints/models/model_list 仅保留 `default` 单分支；2026-07-08 起 8 协议 glm/kimi/minimax/minimax_en/bailian/qianfan/xiaomi_mimo/doubao 的 cp 分支已删，去重 UI「anthropic / anthropic·cp」双显）。2026-07-09 起 GLM Coding Plan 恢复为**独立协议 `glm_coding`**（JSON key `glm_coding`，自带独立 endpoints/models/model_list/peak_hours；base_url `/api/coding/paas/v4` 比普通版 `/api/paas/v4` 多 `/coding/`），前端 `PROTOCOLS` 用独立 `value: "glm_coding"`，Rust `Protocol::GlmCoding`（serde `glm_coding`），双显问题由独立协议根治。其余 7 协议（kimi/minimax/...）仍走旧机制：① endpoint 级 `"coding_plan": false/true` flag（default 端点对象内可标注），② `defaults.ts::pickBranch` 缺 cp 分支自动回落 default（line 102），③ `endpoint.rs` 路由 `has_coding_ep=false` 走普通平台路径，④ 用户级 `platform.extra` 可手工启用 cp 端点（PlatformCard「Code」徽标仍展示）。`injectProtocolHosts` 派生自单一真值，禁抄第二份。
- **peak_hours**（高峰/低峰时段倍率，可选）：JSON 内 per-protocol 条目可加 `peak_hours: [{start_hour,end_hour,multiplier,days_of_week?}]`（UTC+0 基准，多窗口数组，first-match wins，跨天 end<start）；当前未填任何协议实际值（absent = 1.0）。`calc_est_cost` 混合源：`platform.extra.peak_hours`（用户覆盖）→ Rust bundled preset default（`gateway/peak_hours.rs` `OnceLock` 解析）→ 1.0。cost = base × multiplier 落 `proxy_log.est_cost` 单列（无新列）。详见 `.wiki/modules/pricing.md`。
- **disable_during_peak**（高峰期禁用开关，可选）：per-platform 开关 `platform.extra.disable_during_peak`（bool，默认 false）。启用后命中 `peak_hours` 任一窗口时该平台从路由候选排除（与 `expires_at` 同模式：独立维度，不改 status 三态，临时闸门，关开关/出窗口即恢复）。判定复用 `gateway::peak_hours::is_in_peak_window`（与 `resolve_multiplier` 同 hit 逻辑，仅返回 bool）。**单平台组不 bypass**：此开关优先级高于 status bypass（status 维度照旧 bypass auto_disabled / 熔断；高峰禁用维度独立覆盖），单平台组高峰期请求直接 fail。整组所有候选全被高峰排除 → `select_candidates_ctx` 返 `Err("peak_disabled")` → `proxy/handler.rs` route fail 路径落 `proxy_log(blocked_by='router', blocked_reason='peak_hours', status_code=503, est_cost=0)` 审计；其他 NoCandidate 原因（disabled / 熔断无回退）照旧 warn 不落库。前端：`utils/peakHours.ts` `isCurrentlyPeak(windows, nowMs)` 与 Rust 判定对称（cross-layer 一致），PlatformCard 徽标 + formSections 编辑表单「当前: 高峰/非高峰」实时态。

### URL 构造
- `base_url` 含版本前缀（如 `/v1`、`/api/paas/v4`）
- `provider_api_path()` 只返回 `/chat/completions`
- 最终 URL = `base_url + provider_api_path`，禁止额外拼接

### Proxy 日志
- ProxyLogSettings 控制 3 级记录：master switch(enabled) / 用户原始请求(log_user_request) / 上游请求(log_upstream_request)
- 「原始信息」= headers + body + 上游响应正文，**均受 log_user_request / log_upstream_request 开关控制**（gate 在 `from_log`，db/proxy_log.rs）。关开关后这些列入库即清空，**只留解析后元数据**（token / cost / url / status / model 等）。按侧归类：用户侧 (request_headers/request_body/user_response_headers/user_response_body) 受 log_user_request；上游侧 (upstream_request_headers/upstream_request_body/upstream_response_headers/response_body) 受 log_upstream_request。开关开启时入库的 Authorization 等敏感头已脱敏 `[REDACTED]`。例外：流式占位 `"[stream]"` 是控制标记，strip 时保留（终态判定依赖）。
- 3 级 retention：user_request_retention_days(7d) / upstream_request_retention_days(7d) / retention_days(90d)
- retention 清理对称清空整侧「原始信息」（headers + body，UPDATE SET=''），不删行；retention_days 删整行

### Group 统计
- Group 卡片的 usage stats 按 `proxy_log.group_name` 聚合（后端 `get_group_usage_stats` in `db.rs` + command `group_usage_stats` in `lib.rs` + api `groupUsageApi.stats`），只含本分组请求，被多 group 共享的平台不重复计入。前端 Groups.tsx `fetchGroupStats` 对每个 group 调一次。
- balance（余额）维持平台级：关联 platforms 的 `est_balance_remaining` 求和，无 per-group 概念，不按 group_name 拆。

### Local API
- 应用 API 端点以 `/api/` 开头，仅允许 POST 方法
- `POST /api/group-info`：Authorization Bearer `<group_name>` 鉴权，localhost-only
- `GET /` + `GET /proxy`：健康端点，返回 `{"service":"aidog","ok":true}`，无鉴权、不落 proxy_log、跳过组路由（客户端启动探测命中代理根 URL 用，禁删）
- `GET /models` + `GET /v1/models`：总是返回静态默认模型列表（Claude+Codex 官方默认 const），**不依赖 group / token、不 relay 上游**，按 path 协议格式化（含 `/v1/`→openai 列表格式；裸 `/proxy/models`→anthropic 列表格式）。分流前置于 `resolve_group` 之前，tokenless 探测不再 404。仍落 proxy_log(status=200)。静态模型 id 月级腐化需手工核对 `STATIC_MODEL_IDS`（passthrough.rs）。
- statusline bash 脚本通过 `ANTHROPIC_BASE_URL`（推导代理根 URL）+ `ANTHROPIC_AUTH_TOKEN`（= group_name）调用端点
- `settings.{group}.json` 禁止包含 `_aidog_statusline` / `_aidog_subagent_statusline`（`do_sync_group_settings` 会 strip）

## UI / i18n

- 8 种语言（zh-Hans / en-US / ar-SA / fr-FR / de-DE / ru-RU / ja-JP / es-ES），阿拉伯语 RTL
- 主题架构：每主题 light + dark 两组 CSS 变量，位于 `src/themes/`
- UI 风格偏好：Liquid Glass
- 无 react-router：导航是 `App.tsx`(侧栏) + `AppSettings.tsx`(tab) 的本地 state；离页拦截走 `utils/navGuard.ts` 注册表，禁原生 confirm / beforeunload（破坏 Tauri）
- modal/confirm 必须 `createPortal(document.body)`：祖先含 `transform`/`backdrop-filter`（liquid glass 主题）会让 `position:fixed` 退化为相对祖先，弹窗只在 page 内居中。详见 memory `modal-window-center-rule`。
- 数值格式化统一走 `utils/formatters.ts`，禁页内重复定义 formatNumber 等

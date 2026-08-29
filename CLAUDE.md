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
yarn test                     # 前端测试（vitest run，src/utils/*.test.ts 等 18 个测试文件）
cd src-tauri && cargo build   # 仅构 Rust 后端
cd src-tauri && cargo clippy  # Rust lint（warning 必须清）
cd src-tauri && cargo test    # Rust 测试（db/proxy/converter/router/usage_color 等有 #[test]）
```
> 前端测试：18 个 .test.ts/.test.tsx 文件（utils/*.test.ts 10 个 + api.test.ts + defaults.test.ts + 6 个组件测试），run with `yarn test`。

## 项目结构

```
src/                    # React 前端
  pages/                # 页面组件（About/AppSettings/CodexSettings/Groups/Home/Logs/Mcp/ModelInfo/ModelTestPanel/Notifications/Platforms/PopoverConfigTab/Settings/SkillDetailView/SkillInstallView/Skills/Stats/TrayConfigTab）— Settings 为编排容器，子组件见 components/settings/；ModelInfo/ 是「模型信息」页（原 PricingTab 位置，双 tab 列表 + 详情）
  components/
    settings/           # 设置页拆分组件（editors.tsx 全部字段/特殊编辑器 + 令牌 F/S + Header/AnchorNav/UnsavedModal）
    shared/             # 三页共享展示组件（CompactCard/StatChip/BalanceBar/colorScale/usageColor）
  services/api/         # TS 类型定义 + Tauri invoke 封装（目录，非单文件）
    index.ts            # 主入口
    types/              # 类型定义目录
    *.ts                # 各模块 API（groups/platforms/proxy/stats/settings/skills/mcp 等）
  themes/               # 每主题 light/dark CSS 变量
  utils/                # pinyin(拼音搜索) / formatters(统一数值格式化) / navGuard(无路由离页拦截)
src-tauri/              # Rust workspace（aidog_core + aidog_test_util 两 crate）
  crates/
    aidog_core/         # 核心库 + 全部 206 个 #[tauri::command]（准数以 startup.rs 注册表为准）
      gateway/          # models/db/estimate/price_sync/proxy/quota/router/billing/usage_color/peak/time_windows 等
      system_cmd/       # 系统命令（about/app_log/auto_update/backup/notification/scheduling/fs_autocomplete）
      platform_cmd/     # 平台命令（group/platform/quota/stats/price/model_fetch 等）
      proxy_cmd/        # 代理命令（proxy/middleware/mitm/proxy_log/proxy_timeout 等）
      ai_tools_cmd/     # AI 工具命令（coding_tools/mcp/model_test/script_executor/skills 等）
      cli_proxy_cmd/    # CLI 代理命令（batch/import/platform/provider）
      cli_env.rs / settings.rs / defaults.rs / popover.rs / tray_render.rs   # 单文件命令族
      command_macro.rs  # tauri_command! 宏（自动挂 #[tauri::command] + tracing instrument/error）
    aidog_test_util/    # 测试工具（依赖 aidog_core，故 aidog_core 不可反向 dev-dep）
```
> command 注册表在 `src-tauri/src/startup.rs` 的 `tauri::generate_handler![...]`，**它是前端 invoke 名的唯一真值源**（invoke 名取 `#[tauri::command]` 函数名，与模块路径无关）。搬迁命令后用该集合零差集自比对即可证明 invoke 名未变。

## 关键约束

### 平台默认配置 (registry)
- 真值源 = `src-tauri/defaults/registry/`：`index.json`（平台清单 + `last_updated` Unix 秒）+ `platforms/<code>/platform.json`（65 协议，一协议一文件）+ `platforms/<code>/models/<model>.json`（1010 条 per-platform 模型条目）。手维护，禁机器生成覆盖。原 `defaults/platform-presets.json` / `defaults/models.json` / `presets_const.rs` 已废弃删除。
- 平台条目（`platform.json`）字段：`endpoints` / `models` / `model_list` / `peak` + 品牌字段 `name`（8 locale 必填非空）/ `logo_url`（simpleicons slug）/ `color` / `homepage` / `keywords` / `source_urls`（**对象** `{docs, pricing}`，非数组）。**顶层无 `client_type` 字段**（已删 2026-08-29）：端点客户端形态缺省按 endpoint `protocol` 派生（前端 `defaults.ts::clientTypeForProtocol` / Rust `registry.rs::derive_client_type` 对称：anthropic → claude_code、openai 系 → codex_tui、其余 → default），仅例外平台在 endpoint 显式标注（如官方 claude_code 直连端点标 default 不模拟客户端）。另可选 `key_prefixes`（平台 API key 前缀数组，如 sk-ant- / ark- / tp-，**唯一**前缀字段——已并入旧 codingKeyPrefixes，coding 套餐平台是独立协议、专属 token 前缀直接写自家 `key_prefixes`）：粘贴识别的 key 提取正则与平台直判（优先级 2）由 `collectKeyPrefixes` 据此数据驱动生成，**平台前缀禁在代码硬编码**（通用 sk- / sk_ 除外）。`keywords` 中文词条须附全拼 + 首字母字面串（如 智谱 → zhipu + zp），拼音形式存数据、代码不做推导。模型条目（`models/<model>.json`）字段：`model_id`（平台真实请求名）/ `display_name`（单字符串不译，缺省回落 `model_id`）/ `canonical_model`（跨平台聚合键）/ `family` / `version` / `predecessor` / `capabilities` / `builtin_tools_excluded` / `max_input_tokens` / `max_output_tokens` / `context_window` / `official` / 三价字段 / `peak`（高峰**绝对价**）/ `time_tiers` / `context_tiers`。同一模型每平台一条独立条目，靠 `canonical_model` 关联。
- Rust 入口 `aidog_db::registry`：`build.rs` 编译期枚举全部文件 → `include_str!`；`presets()` / `presets_json()` 合并出旧 presets 文档形状供 `defaults.rs::get_defaults_json` 回传前端。模型侧**无合并视图**（旧 `models.json` 单模型归并形状随票 T6 删除），只出 `bundled_model_files()` 原始文本。无 app data 覆盖层。
- **DB-only 运行时**（ADR 0005）：两张镜像表 `model_entry`（主键 `platform_code + model_id`）与 `platform_preset`（主键 `code`），远程同步直接 upsert 入库，`~/.aidog/` 无 JSON 缓存层（老 `~/.aidog/platform-presets.json` 忽略不迁移）。bundled include 仅在 DB 整表空时兜底（`get_model_entry` / `list_model_entries` / `defaults.rs`）。旧 `model_price` 表已 DROP（migration 20260826-03）。
- 远程同步 `gateway/price_sync.rs::sync_registry`（command 名仍是 `model_price_sync`，理由见 `platform_cmd/price.rs` 顶部注释）：jsDelivr 主源 + raw.githubusercontent 兜底，**index 驱动**（先拉 `index.json`，失败即整轮放弃）+ **best-effort 逐文件**（单文件两源全败 → 记进 `failures` 清单并**保留 DB 旧行**，不清空不部分覆盖），16 路并发。
- 计费解析 `aidog_db::resolve_price`（ADR 0006，顺序不可换）：① 命中 preset `peak` 窗口且条目带 `peak` → 用模型 **peak 绝对价**，此时 `PriceResolution::peak_applied=true`，调用方**不得再乘平台倍率**（否则双重计价）→ ② 否则条目默认价（含 `time_tiers` / `context_tiers` 分档）× 平台 `peak` 倍率 → ③ 条目缺失 → `PriceSyncSettings` 的 fallback 单价（默认 3.0 $/M，不返回 0）。
- 前端函数（`src/domains/platforms/defaults.ts`）：`getDefaultEndpoints` / `getDefaultModels` / `getDefaultModelList` / `getDefaultPeak` **async**（模块级 `docPromise` 单次 RPC 缓存，多次调用复用同一 IPC；**所有 caller 必须 `await`**，TS 编译捕获漏 await）；`defaultClientForProtocol`（= `clientTypeForProtocol`）**sync** 纯派生，无 RPC（registry 已删 client_type 字段）。`getDefaultModels` 第 2 参 `isPeak`（PRD 07-11）：true 且 preset 含 `models.peak` 分支 → 返 peak 映射；缺省 false（向后兼容）。models 走 `pickModelsBranch`（两分支 default/peak），endpoints/model_list 走 `pickBranch`（单分支 default，model_list.default 必须是 models 各分支值集的超集 —— 见下 peak 段）。
- mock 协议不在 JSON 内（`platformPaste.ts:15` 从 `matchPlatform` 排除）；`getDefaultEndpoints("mock")` 返 `[]`。
- **coding 套餐一律独立协议，无 coding_plan 分支**：preset JSON **无** `coding_plan` 子分支（endpoints/models/model_list 仅 `default`，models 另有可选 `peak`；schema 已删 cp 分支定义，2026-08-29）。所有 coding 套餐平台（glm_coding / glm_coding_en / kimi_coding / minimax_coding / xiaomi_mimo_coding / xiaomi_mimo_coding_en / bailian_coding / bailian_coding_en / qianfan_coding / compshare_coding）是独立协议条目，JSON 顶层 `is_coding_plan: true` 标记（Rust `router/ordering.rs` 排序 + 前端徽标/选择器派生）。glm_coding base_url `/api/coding/paas/v4` 比普通版 `/api/paas/v4` 多 `/coding/`，Rust `Protocol::GlmCoding`（serde `glm_coding`）。运行时 endpoint 级 `coding_plan` flag 保留（`endpoint.rs` 路由 `has_coding_ep`、`forward.rs` cp 注入；preset endpoints 里 coding 协议端点标 true，用户级 `platform.extra` 可手工启用，PlatformCard「Code」徽标仍展示）。`injectProtocolHosts` 派生自单一真值，禁抄第二份。
- **peak**（高峰/低峰时段倍率，可选；2026-08-29 由 `peak_hours` 全链路更名，DB 存量键启动迁移 20260829-02）：JSON 内 per-protocol 条目可加 `peak: [{start_hour,end_hour,multiplier,days_of_week?,models?,start_at?,timezone?}]`（`timezone` IANA 名缺省=UTC；小时/星期/日期按该时区**本地时刻**解释，Rust `wall_time`(chrono-tz) ↔ TS `wallTimeInTz`(Intl) 跨层对称；多窗口数组，first-match wins，跨天 end<start；`models` 限定仅特定请求模型命中窗口，`start_at` Unix 秒窗口生效起点）。现带实际值：glm_coding（北京工作日 14-18 ×3.0，另 0-24 ×2.0 带 start_at）、deepseek（条目价=官方 off-peak，工作日北京 9-12/14-18 ×2 限 `deepseek-v4*`）、crazyrouter / opencode_zen（北京 9-12/14-18，对齐 DeepSeek 官方峰时）、siliconflow（北京 17-次日2 ×1.25）；其余 absent = 1.0。`calc_est_cost` 混合源：`platform.extra.peak`（用户覆盖）→ Rust bundled preset default（`gateway/peak.rs` `OnceLock` 解析）→ 1.0。cost = base × multiplier 落 `proxy_log.est_cost` 单列（无新列）。**注意与模型 `peak` 绝对价的优先级**：条目带 `peak` 时倍率被压成 1.0（见上「计费解析」），只有条目无 `peak` 才走本条倍率链。
- **models.peak**（高峰时段模型切换分支，PRD 07-11，可选）：per-protocol `models` 可加 `peak` 分支（与 `default` 并列；仅 glm_coding 现带）。路由层 `gateway/router/candidates.rs::resolve_effective_models` 三层级联：① `time_windows`（用户显式时段切换，优先级最高）→ ② `preset.models.peak`（用户未配 time_windows + 命中 `peak_for` 任一窗口 → 用 peak 替换 effective_models）→ ③ `platform.models` 兜底。**peak 为 preset 级硬约束，设计上覆盖用户手工定制的 `platform.models`**（用户如需保留自定义请配 `time_windows`）。`default_peak_models`（`peak.rs`）从 bundled preset 读 peak 分支，与 `default_peak` 同 OnceLock 同 idiom。前端 cross-layer：`PlatformCard` 算 `isPeak = isCurrentlyPeak(userPh ?? preset default)` 传 `getDefaultModels(isPeak)` 仅影响展示；后端 `resolve_effective_models` 按请求 `source_model` 路由（含 model scope 过滤）。**不变量：`model_list.default` 必须是 `models` 各分支（default/peak）值集的超集**（下拉冷启动兜底列全候选；当前不加 `model_list.peak` 独立分支，default 已覆盖，YAGNI）。
- **disable_during_peak**（高峰期禁用开关，可选）：per-platform 开关 `platform.extra.disable_during_peak`（bool，默认 false）。启用后命中 `peak` 任一窗口时该平台从路由候选排除（与 `expires_at` 同模式：独立维度，不改 status 三态，临时闸门，关开关/出窗口即恢复）。判定复用 `gateway::peak::is_in_peak_window`（与 `resolve_multiplier` 同 hit 逻辑，仅返回 bool）。**单平台组不 bypass**：此开关优先级高于 status bypass（status 维度照旧 bypass auto_disabled / 熔断；高峰禁用维度独立覆盖），单平台组高峰期请求直接 fail。整组所有候选全被高峰排除 → `select_candidates_ctx` 返 `Err("peak_disabled")` → `gateway/proxy/handler.rs` route fail 路径落 `proxy_log(blocked_by='router', blocked_reason='peak', status_code=503, est_cost=0)` 审计（写入点 2026-08-29 起统一为 `peak`，历史行旧值 `peak_hours` 保留不回填）；其他 NoCandidate 原因（disabled / 熔断无回退）照旧 warn 不落库。前端：`utils/timeWindow.ts` `isCurrentlyPeak(windows, nowMs)` 与 Rust 判定对称（cross-layer 一致），PlatformCard 徽标 + formSections 编辑表单「当前: 高峰/非高峰」实时态。

### URL 构造
- `base_url` 含版本前缀（如 `/v1`、`/api/paas/v4`）
- `provider_api_path()` 只返回 `/chat/completions`
- 最终 URL = `base_url + provider_api_path`，禁止额外拼接

### Proxy 日志
- ProxyLogSettings 控制 3 级记录：master switch(enabled) / 用户原始请求(log_user_request) / 上游请求(log_upstream_request)
- 「原始信息」= headers + body + 上游响应正文，**均受 log_user_request / log_upstream_request 开关控制**（gate 在 `from_log`，`gateway/models/proxy_log.rs` + `gateway/db/proxy_log.rs`）。关开关后这些列入库即清空，**只留解析后元数据**（token / cost / url / status / model 等）。按侧归类：用户侧 (request_headers/request_body/user_response_headers/user_response_body) 受 log_user_request；上游侧 (upstream_request_headers/upstream_request_body/upstream_response_headers/response_body) 受 log_upstream_request。开关开启时入库的 Authorization 等敏感头已脱敏 `[REDACTED]`。流式日志终态由显式 `done` 列判定（终态 = status!=0 且 done=1；流式 flush/断连兜底/非流式终态/中断补写置位），`[stream]` 占位哨兵已废（2026-08-24 票 06），body 列不承载控制语义。
- 3 级 retention：user_request_retention_days(7d) / upstream_request_retention_days(7d) / retention_days(90d)
- retention 清理对称清空整侧「原始信息」（headers + body，UPDATE SET=''），不删行；retention_days 删整行

### Group 统计
- Group 卡片的 usage stats 按 `proxy_log.group_name` 聚合（后端 `get_group_usage_stats` in `gateway/db/group.rs` + command `group_usage_stats` in `commands_platform` + api `groupUsageApi.stats`），只含本分组请求，被多 group 共享的平台不重复计入。前端 Groups.tsx `fetchGroupStats` 对每个 group 调一次。
- balance（余额）维持平台级：关联 platforms 的 `est_balance_remaining` 求和，无 per-group 概念，不按 group_name 拆。

### Local API
- 应用 API 端点以 `/api/` 开头，仅允许 POST 方法
- `POST /api/group-info`：Authorization Bearer `<group_name>` 鉴权，localhost-only
- `GET /` + `GET /proxy`：健康端点，返回 `{"service":"aidog","ok":true}`，无鉴权、不落 proxy_log、跳过组路由（客户端启动探测命中代理根 URL 用，禁删）
- `GET /models` + `GET /v1/models`：总是返回静态默认模型列表（Claude+Codex 官方默认 const），**不依赖 group / token、不 relay 上游**，按 path 协议格式化（含 `/v1/`→openai 列表格式；裸 `/proxy/models`→anthropic 列表格式）。分流前置于 `resolve_group` 之前，tokenless 探测不再 404。仍落 proxy_log(status=200)。静态模型 id 月级腐化需手工核对 `STATIC_MODEL_IDS`（`gateway/proxy/passthrough.rs`）。
- statusline bash 脚本通过 `ANTHROPIC_BASE_URL`（推导代理根 URL）+ `ANTHROPIC_AUTH_TOKEN`（= group_name）调用端点
- `settings.{group}.json` 禁止包含 `_aidog_statusline` / `_aidog_subagent_statusline`（`do_sync_group_settings` 会 strip）

## UI / i18n

- 8 种语言（zh-Hans / en-US / ar-SA / fr-FR / de-DE / ru-RU / ja-JP / es-ES），阿拉伯语 RTL
- 主题架构：每主题 light + dark 两组 CSS 变量，位于 `src/themes/`
- UI 风格偏好：Liquid Glass
- 无 react-router：导航是 `App.tsx`(侧栏) + `AppSettings.tsx`(tab) 的本地 state；离页拦截走 `utils/navGuard.ts` 注册表，禁原生 confirm / beforeunload（破坏 Tauri）
- modal/confirm 必须 `createPortal(document.body)`：祖先含 `transform`/`backdrop-filter`（liquid glass 主题）会让 `position:fixed` 退化为相对祖先，弹窗只在 page 内居中。详见 memory `modal-window-center-rule`。
- 数值格式化统一走 `utils/formatters.ts`，禁页内重复定义 formatNumber 等

## Agent skills

### Issue tracker

票以 markdown 文件存 `.scratch/<feature>/`（不用 GitHub Issues，避免与 `.skein/` 形成双任务系统对不上账）。见 `docs/agents/issue-tracker.md`。

### Triage labels

五个默认角色标签（needs-triage / needs-info / ready-for-agent / ready-for-human / wontfix），未改名。见 `docs/agents/triage-labels.md`。

### Domain docs

single-context：根 `CONTEXT.md` + `docs/adr/`（按需惰性创建，缺失不报错）。见 `docs/agents/domain.md`。

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

### URL 构造
- `base_url` 含版本前缀（如 `/v1`、`/api/paas/v4`）
- `provider_api_path()` 只返回 `/chat/completions`
- 最终 URL = `base_url + provider_api_path`，禁止额外拼接

### Proxy 日志
- ProxyLogSettings 控制 3 级记录：master switch / 用户原始请求 / 上游请求
- `*_headers`（元数据，Authorization 已脱敏 `[REDACTED]`）始终入库、不受 log_user_request / log_upstream_request 开关控制；仅 `*_body`（prompt / 响应正文）受开关控制
- 3 级 retention：user_request_retention_days(7d) / upstream_request_retention_days(7d) / retention_days(90d)
- retention 清理只清空 `*_body` 字段（UPDATE SET=''），不删行，不清 headers；retention_days 删整行

### Group 统计
- Group 卡片的 usage stats 按 `proxy_log.group_name` 聚合（后端 `get_group_usage_stats` in `db.rs` + command `group_usage_stats` in `lib.rs` + api `groupUsageApi.stats`），只含本分组请求，被多 group 共享的平台不重复计入。前端 Groups.tsx `fetchGroupStats` 对每个 group 调一次。
- balance（余额）维持平台级：关联 platforms 的 `est_balance_remaining` 求和，无 per-group 概念，不按 group_name 拆。

### Local API
- 所有 API 端点以 `/api/` 开头，仅允许 POST 方法
- `POST /api/group-info`：Authorization Bearer `<group_name>` 鉴权，localhost-only
- statusline bash 脚本通过 `ANTHROPIC_BASE_URL`（推导代理根 URL）+ `ANTHROPIC_AUTH_TOKEN`（= group_name）调用端点
- `settings.{group}.json` 禁止包含 `_aidog_statusline` / `_aidog_subagent_statusline`（`do_sync_group_settings` 会 strip）

## UI / i18n

- 7 种语言（zh-CN / en-US / ar-SA / fr-FR / de-DE / ru-RU / ja-JP），阿拉伯语 RTL
- 主题架构：每主题 light + dark 两组 CSS 变量，位于 `src/themes/`
- UI 风格偏好：Liquid Glass
- 无 react-router：导航是 `App.tsx`(侧栏) + `AppSettings.tsx`(tab) 的本地 state；离页拦截走 `utils/navGuard.ts` 注册表，禁原生 confirm / beforeunload（破坏 Tauri）
- 数值格式化统一走 `utils/formatters.ts`，禁页内重复定义 formatNumber 等

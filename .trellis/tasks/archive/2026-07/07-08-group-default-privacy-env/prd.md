# 新建 group 预填隐私 env 默认值

## Goal

aidog 生成的 Claude Code 配置默认禁遥测/增长/反馈。新建 group 时预填一批隐私 env 到 env_vars，per-group 可改可删（不强注入不可关）。

## 用户指定 env（9 个）

- CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1
- CLAUDE_CODE_ENABLE_TELEMETRY=0
- CLAUDE_CODE_ENHANCED_TELEMETRY_BETA=0
- CLAUDE_CODE_DISABLE_FEEDBACK_SURVEY=1
- CLAUDE_CODE_BYOC_ENABLE_DATADOG=0
- CLAUDE_CODE_PROPAGATE_TRACEPARENT=0
- DISABLE_GROWTHBOOK=1
- CLAUDE_CODE_ATTRIBUTION_HEADER=0
- DISABLE_INSTALLATION_CHECKS=1

## Scope (best judgment, 用户超时裁定)

- **落点**: 新建 group form 初始 env_vars 预填这 9 个（前端 Groups.tsx / editors.tsx）
- **i18n**: 5 个未在 locales 的补 8 语言 label+desc：
  - DISABLE_GROWTHBOOK / DISABLE_INSTALLATION_CHECKS / CLAUDE_CODE_BYOC_ENABLE_DATADOG / CLAUDE_CODE_DISABLE_FEEDBACK_SURVEY / CLAUDE_CODE_PROPAGATE_TRACEPARENT
- 已在 locales 的 4 个不重复：CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC / ENABLE_TELEMETRY / ENHANCED_TELEMETRY_BETA / ATTRIBUTION_HEADER

## Requirements

1. 定位新建 group form 初始值代码（grep setEditingGroup / useState.*GroupConfig / 新建 group 按钮回调），env_vars 默认从 `[]` 改为预填 9 个 `{key, value}`
2. 预填仅在「新建」时触发，编辑已有 group 不覆盖用户配置
3. 5 个新 env 补 i18n（8 locale × 5 env × 2 字段 = 80 条），格式 `env.<KEY>` + `env.<KEY>.desc`
4. 前端 env 编辑器渲染新 env 用 i18n label

## Out of Scope

- 不改 Rust sync_settings.rs（注入逻辑不变）
- 不强注入全局 settings.json
- 不动 claude_code 协议 preset

## Acceptance Criteria

- [ ] 新建 group env_vars 预填 9 个隐私 env
- [ ] 编辑已有 group 不覆盖
- [ ] 8 locale 补全 5 env × label+desc
- [ ] `yarn build` 通过
- [ ] `cargo check` 通过（若改 Rust）
- [ ] check:i18n 通过（无裸 key）

## Technical Notes

- env i18n 范本: `src/locales/zh-Hans.json:122` (CLAUDE_CODE_ATTRIBUTION_HEADER label+desc)
- env_vars 结构: `Vec<EnvVar{key, value}>` (src-tauri/src/gateway/models/group.rs:41)
- locales: src/locales/{zh-Hans,en-US,ar-SA,fr-FR,de-DE,ru-RU,ja-JP,es-ES}.json
- 新建 group form: Groups.tsx 或 components/settings/editors.tsx（grep 定位）

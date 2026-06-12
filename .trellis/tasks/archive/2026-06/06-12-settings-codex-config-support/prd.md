# Codex 配置支持（设置页 + 分组 + 平台，对照 Claude Code 体系）

## Goal

在 aidog 加入 OpenAI Codex CLI 配置支持，三层（对照现有 Claude Code 实现）：① 设置页全局 Codex 配置 + 推荐默认 ② 每分组独立 Codex 配置生成 ③ 每 AI 平台独立 Codex 配置 override。Codex 配置格式为 TOML（`~/.codex/config.toml`）。

## Requirements（初定，待研究 + brainstorm 收敛）

### 全局设置（对照 claude_code 设置 + RECOMMENDED_CONFIG）
- R1 设置页新增「Codex」区/页：schema 驱动 UI（对照 claude-settings-schema.ts），编辑 Codex config 字段。
- R2 推荐默认 Codex config（对照 RECOMMENDED_CONFIG / defaults/settings.json），含「指向 aidog 本地代理」相关项（model provider base_url 指向本地 proxy）。
- R3 后端 TOML 读写 ~/.codex/config.toml（Rust toml crate）+ Tauri commands（对照 settingsApi/claudeSettingsImportApi）。

### 分组级（对照 do_sync_group_settings → settings.{group}.json）
- R4 每分组生成独立 Codex 配置（profile 或 per-group 文件），路由指向该分组的 aidog 代理端点（对照 ANTHROPIC_BASE_URL/AUTH_TOKEN env 注入）。Codex 用 model_provider/profile 机制实现（待研究确认）。

### 平台级（对照平台的 Claude Code Config Override）
- R5 每 AI 平台可设独立 Codex 配置 override（对照 Platforms 编辑页的 Config Override / endpoints）。

## Research

- Codex config schema + 推荐默认 + provider/profile 机制 见 `research/codex-config.md`（研究 agent 抽取 5 篇官方文档：config-basic/advanced/reference/environment-variables/config-sample）。

## Open Questions（brainstorm）

- 全局 Codex 设置放设置页哪（新顶级 tab「Codex」vs Claude Code 设置页内分区）？
- 分组 Codex 生成机制：Codex 是否支持 per-profile/per-provider（对照 Claude Code per-group settings 文件）？路径/格式？
- 平台 override 粒度：override 哪些 Codex 字段？
- 推荐默认具体值（待研究输出）。
- 是否复用现有 do_sync_group_settings 流程加 Codex 输出，还是独立流程。

## Technical Notes

- Claude Code 现有实现参考：
  - schema：`src/services/claude-settings-schema.ts`（SECTIONS + RECOMMENDED_CONFIG，已统一派生自 defaults/settings.json）。
  - 设置页：`src/pages/Settings.tsx` + `components/settings/`。
  - 分组生成：`lib.rs::do_sync_group_settings` → `~/.aidog/settings.{group}.json`。
  - 平台 override：Platforms 编辑页 Config Override + endpoints。
- Codex：TOML，`~/.codex/config.toml`；需 Rust toml 解析。
- **依赖**：实施改 Settings.tsx/lib.rs/Platforms.tsx/Groups.tsx → 与 statusline 任务共享文件，须等其合并后实施。大功能，研究 + brainstorm 后**拆 parent + child**（全局/分组/平台 三交付可分阶段）。

### 分组复制命令（R6，对照 Claude Code copy command）
- R6.1 Groups 列表加「复制 Codex 命令」按钮：生成可直接用的 `codex ...` 命令，指向该分组的 Codex 配置（profile）。
- R6.2 命令尽可能启用功能 + 启用 bypass：`codex -p <group> --dangerously-bypass-approvals-and-sandbox -a never [--enable <feat>...]`（CLI 参数见 research/codex-cli.md，实测 codex --help）。
- R6.3 对照现有 Groups 的 Claude Code copy command 实现（`claude --settings ~/.aidog/settings.{group}.json`）。

## Decisions（brainstorm 已定）

- **先做验证 spike**：实施 MVP 前先验证 codex provider 指向 aidog 代理 + Responses 端到端可行（3 个 caveat），通过再实施。
- **MVP 范围**：① 全局 Codex 设置页（新顶级 Tab「Codex」）+ 推荐默认 ② 分组生成 Codex profile + 「复制 Codex 命令」(bypass)。**平台 override (R5) 推迟二期**。
- **设置页位置**：新增顶级 Tab「Codex」，与「Claude Code 设置」并列，各自 schema 驱动（JSON vs TOML 隔离）。

## 调度（待 spike 通过后）

```mermaid
graph LR
  S[Spike: 验证 codex→aidog Responses 链路] --> D1[全局 Codex 设置 Tab + 推荐默认 + TOML 读写]
  S --> D2[分组生成 Codex profile + 复制命令(bypass)]
  D2 -.依赖.-> D1
  P2[二期: 平台 override] -.推迟.-> D1
```

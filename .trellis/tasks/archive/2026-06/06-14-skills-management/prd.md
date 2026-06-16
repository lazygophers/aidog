# 基于 npx skills 的 skills 管理模块

## Goal

在 aidog 桌面应用中新增顶层「Skills」模块，让用户在 GUI 内浏览 / 搜索 / 安装 / 列出 / 更新 / 卸载 agent skills（基于 Vercel Labs 的 `npx skills` 生态 + skills.sh catalog），与 aidog 已有的 Claude Code / Codex 配置管理形成闭环。

## Requirements

### 实现方式（混合）
- **读操作** Rust 原生：扫描本地已装 skills 目录、抓 skills.sh catalog / GitHub。
- **写操作** shell out `npx skills`：`add` / `update` / 卸载（删目录或 npx 等价）。
- 复用 `std::process::Command`（参考 `gateway/notification.rs:264`）。

### 安装目标（默认用户级 + 可选项目）
- **默认 scope = 用户级全局**（`npx skills add -g`，装到 `~/.claude/skills` 等）。
- 允许用户**选择一个项目目录**来管理该项目的 skills（项目级，不带 `-g`，装到 `<project>/.claude/skills`）。
- **target agent 可选**：默认 Claude Code；UI 提供 agent 选择（Claude Code / Codex / Cursor…，对应 `npx skills --agent <agent>`）。

### 操作范围（MVP 全量）
- **浏览 / 搜索 catalog**：拉 skills.sh catalog + `npx skills find <kw>` 搜索，展示可装 skills。
- **安装 skill**：`npx skills add <owner/repo> [--agent <a>] [-g | 项目目录]`。
- **列已装 + 卸载**：原生扫描目标 skills 目录列出已装，支持删除。
- **更新**：`npx skills update` 更新已装 skills。

### UI 位置
- 顶层侧栏新页 "Skills"（App.tsx navItems 加项，与 Platforms/Groups/Logs 平级）。

### 环境探测
- 启动/进入页时探测 `npx` / `node` 是否可用；缺失 → 友好提示 + 引导（不崩）。

## Acceptance Criteria

- [ ] 顶层侧栏出现 "Skills" 页，可进入
- [ ] 能浏览 + 搜索 catalog 中的 skills
- [ ] 能安装指定 skill 到（默认用户级 / 选定项目）+ 选定 agent
- [ ] 能列出目标 scope 下已装 skills 并卸载
- [ ] 能一键更新已装 skills
- [ ] npx/node 缺失时给明确提示，不崩溃
- [ ] 7 语言 i18n 覆盖（zh-CN/en-US/ar-SA/fr-FR/de-DE/ru-RU/ja-JP）
- [ ] Rust `cargo clippy` 无 warning + `cargo test` 绿；前端 `yarn build` 绿

## Definition of Done

- Rust `cargo clippy` 无 warning + `cargo test` 绿
- 前端 `yarn build`（tsc && vite build）绿
- i18n 7 语言补全
- 文档/注释随行为更新；非平凡发现落 cortex

## Technical Approach

- **后端** `src-tauri/src/`：新增 `gateway/skills.rs`（catalog 抓取 + 本地扫描 + npx 调用封装），lib.rs 注册 commands（`skills_list_installed` / `skills_browse_catalog` / `skills_search` / `skills_install` / `skills_remove` / `skills_update` / `skills_check_env`）。
- **前端** `src/pages/Skills.tsx` + `services/api.ts` 封装 + App.tsx navItems + i18n。
- **shell out**：`std::process::Command::new("npx").args(["skills", ...])`，捕获 stdout/stderr + 退出码，解析结果回前端。
- **catalog**：优先原生 HTTP 抓 skills.sh（或 GitHub 检索），失败回退 `npx skills find`。
- **scope/agent 状态**：前端持有 target scope（global | project path）+ agent 选择，传入后端命令。

## Decision (ADR-lite)

- **Context**: aidog 需在 GUI 内管理 agent skills，需在「复用官方工具」与「原生可控」间取舍。
- **Decision**: 混合方案 — 读原生（快、不依赖 CLI 输出格式）、写 shell out `npx skills`（兜底正确性 + 复用官方生态）；默认用户级 scope，可选项目级；顶层侧栏新页。
- **Consequences**: 写操作依赖本机 node/npx（需探测+提示）；catalog 抓取需跟 skills.sh 结构对齐（加 npx find 回退）；后续可扩展 init 脚手架 / lock 文件管理。

## Out of Scope

- `npx skills init`（脚手架新 skill）—— 后续迭代
- lock 文件 / `experimental_install` / `experimental_sync` 同步
- 自动定时更新 skills
- skill 内容编辑器

## Technical Notes

- 接入参考：navItems(`src/App.tsx`) / `src/pages/` / lib.rs commands(73) / `src/services/api.ts` / `src/themes/` i18n
- shell out 参考：`src-tauri/src/gateway/notification.rs:264`（`Command::new("say")`）
- npx skills 源：https://github.com/vercel-labs/skills, https://skills.sh
- 命令速查：`add <owner/repo>` `list`/`ls`(`-g`,`-a <agent>`) `find <kw>` `update` `--agent <a>`

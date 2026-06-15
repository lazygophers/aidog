# Skills 搜索结果与 npx 命令输出不一致修复

## Goal

aidog「添加 Skills 搜索」给 `lazygophers/ccplugin` 仅返回 **6 项**（skills.sh 外部索引），而 `npx skills add lazygophers/ccplugin -l` 显示仓库实际有 **11 项**可装。用户期望两者一致。

## 根因（已诊断）

- **aidog 后端**（`skills.rs::search` → `npx_find`）走 `npx skills find <kw>`，命中 **skills.sh** 三方索引 (`https://skills.sh/<owner>/<repo>/<skill>`)——**只收录被安装过 / 有 install 计数的 skill**，新仓库 / 新 skill 缺位（6/11）。
- **npx skills add <source> -l** 直接 `git clone` 仓库 → scan SKILL.md → 列**真实可装集**（11/11）。
- 「源头不同」= 搜索结果天然不一致；当用户搜的关键词已是精确 `owner/repo` 形态时，应当走真实 catalog 列举而非 skills.sh 索引。

## Decision (ADR-lite)

**双模式 search**：
- 关键词匹配 `^[a-zA-Z0-9._-]+/[a-zA-Z0-9._-]+$` 即「精确 owner/repo 形态」→ 走 `npx skills add <source> -l --yes`，解析输出列**仓库全部可装 skill**。
- 关键词为其他形态（普通搜索词、含 `@` 后缀、URL 等）→ 保持现有 `npx skills find <kw>` 走 skills.sh。
- 两路径统一返回 `Vec<CatalogEntry>`，前端无感。

**add -l 输出解析**：
- 输出含 spinner（◒/◐/◓/◑/●/◇）+ 框形字符（│）+ ANSI 颜色。先 ANSI 剥 + 行级清理。
- 每条 skill 占多行：`│    <skill-name>\n│\n│      <description>\n│`。
- 提取 `name`（缩进 4 空格的非空行）+ `description`（缩进 6 空格的后续行）。
- `id` = `<source>@<skill-name>`（与 find 输出形态一致，前端 add 时直接用）。
- `repo_url` = `https://github.com/<source>` （source 解析自传入参数）。

## Requirements

- 后端 `skills.rs::npx_find` 前加 source 形态检查：精确 source → 调 `npx_list_source(source, proxy)`。
- 新增 `fn npx_list_source(source: &str, proxy_url: Option<&str>) -> Vec<CatalogEntry>`：
  - 跑 `npx --yes skills add <source> -l -y`（`-y` 自动接受 prompt，否则可能挂）
  - 关闭 stdin
  - apply_proxy_env
  - 解析输出
- 新增 `parse_add_list_output(raw: &str, source: &str) -> Vec<CatalogEntry>` + 单测覆盖：
  - 解析 typical fixture（含 spinner + 框形）→ 11 条
  - 空输入 → 空
  - 部分残缺 → 跳过坏行不崩
- 现有 `npx_find` 兼容（混合关键词时仍走 find）。
- cargo clippy / test / yarn build 全绿。

## Acceptance Criteria

- [ ] 后端 search("lazygophers/ccplugin") 返回 11 条（与 `npx skills add -l` 一致），而不是 6 条。
- [ ] 后端 search("trellis") 仍走 find（无 `/` 形态）。
- [ ] 后端 search("vercel-labs/agent-skills") → 走 add -l 解析（与 `npx skills add vercel-labs/agent-skills -l` 一致）。
- [ ] 输出 `CatalogEntry.id` 形如 `owner/repo@skill`，前端 add 时直接用。
- [ ] `parse_add_list_output` 单测含 fixture 覆盖 ANSI / spinner / 多空行 / 残缺。
- [ ] cargo test / clippy / build 全绿。

## Definition of Done

- worktree commit + merge + archive
- cortex memory 落档（add -l vs find 数据源差异 + 解析规则）

## Out of Scope

- 改 add 走自己 git clone 缓存（用 npx skills add 即可）
- search 结果合并（add -l + find 取并集）— 当前精确 source 走 add -l 已能拿到全集
- npx 跑超时配置（保留 std::process::Command 默认）
- skills.sh 索引覆盖率优化（外部 SaaS 不归我们）

## Files

- `src-tauri/src/gateway/skills.rs` — search 分流 + npx_list_source + parse_add_list_output + 单测

## Research References

无外部研究 — 内部命令对比 + 现有 parse_find_output 镜像。

# 默认分组导入 diff 托管字段区分

## 背景 / Bug
设某分组为「默认分组」→ `do_sync_group_settings` → `write_default_claude_settings`（`src-tauri/src/commands/sync_settings.rs`）把默认组 config deep-merge 写入 `~/.claude/settings.json`。注入字段：`env.ANTHROPIC_BASE_URL`/`env.ANTHROPIC_AUTH_TOKEN`、`hooks`(marker 开启时)、以及整个 `base_config`（含 `enabledPlugins`/`statusLine` 等）。

随后用户在「设置 → 从 Claude Code 导入」（`src/pages/Settings.tsx:224 handleImportFromClaudeCode` → `read_claude_code_settings` 读 live 文件 → `buildImportDiffTree(config, source)` 字段级 diff，`config`=DB `settings("global","claude_code")` 或前端回退 `RECOMMENDED_CONFIG`）会看到大量「变更」（含插件），预期应零差异。

### 根因（双重）
1. **注入字段污染**：`env.ANTHROPIC_BASE_URL/AUTH_TOKEN`、`hooks` 等 aidog 写入 live 文件但不在 DB 基线 `config` → diff 恒显示。
2. **前后端 fallback 不对称**：DB `claude_code` 为空时，写入侧 `base_config` 回退 `defaults/settings.json`（含 `enabledPlugins`），前端 `config` 回退 `RECOMMENDED_CONFIG`（无 `enabledPlugins`）→ 即使用户零自装插件，defaults 的 `enabledPlugins` 也现为「变更」。

`buildImportDiffTree`（`src/components/settings/editors.tsx:2531`）已跳过 `_aidog_*` 顶层 key，但无「托管字段」概念。

## 功能本意
导入功能 = 用户用命令自装别的东西（插件 / mcpServers 等）后，**只导入差异（用户自加）的部分**。当前缺陷 = 无法区分「aidog 默认分组写入的托管配置」vs「用户自己加的」。

## 已锁定设计决策（用户经 AskUserQuestion 确认）
- **修复方向 = 双管齐下**：A 写入侧收敛 + B 比对侧字段级 diff。
- **托管边界 = 都要管**：aidog 默认分组应把 `enabledPlugins`(plugins) + `mcpServers` **纳入托管模型**用于正确区分 diff，**非覆盖用户自装的** —— 用户命令自装的插件必须保留、且在导入 diff 中正常列出。

## 需求
### A — 写入侧收敛（`sync_settings.rs`）
1. 默认分组写 `~/.claude/settings.json` 时，记录 aidog **实际注入/托管**的字段路径集（dot-path），写入 `_aidog_managed` marker（`_aidog_` 前缀，自动被前端隐藏；CC 忽略未知 key）。托管集至少含：`env.ANTHROPIC_BASE_URL`、`env.ANTHROPIC_AUTH_TOKEN`、`statusLine`、`hooks`、`enabledPlugins`、`mcpServers`（实际有写入的才记）。
2. `enabledPlugins` / `mcpServers` 用**并集 merge**（不覆盖用户自装条目）；只把 aidog 自身注入的条目记入托管集。用户自装条目不进托管集 → 导入 diff 仍能列出。
3. 修前后端 fallback 不对称：使「无 DB config 时」前后端基线一致（推荐：前端导入 diff 基线改用「后端实际托管集 / 后端回退」而非 `RECOMMENDED_CONFIG`，或令两端 fallback 同源）。

### B — 比对侧字段级 diff（`buildImportDiffTree` / `Settings.tsx`）
1. diff 时读 `source`（live 文件）的 `_aidog_managed` marker，**排除托管路径**，只列非托管（用户自加）字段差异。
2. 托管集为空（无默认分组 / 旧文件无 marker）时退化为现有行为，零回归。
3. 嵌套路径（如 `env.ANTHROPIC_BASE_URL`）的排除需精确到子键，不能整把 `env` 排掉（用户可能自加 `env.FOO`）。

### 单一事实源
「托管字段定义」需 Rust 写入侧与前端 diff 侧一致。优先用 `_aidog_managed` marker 由写入侧落盘、前端读取（运行时真值，避免双写常量漂移）。

## 验收标准
1. 设默认分组后立即「从 Claude Code 导入」→ 提示「无差异」（零托管字段泄漏到 diff）。
2. 用户用命令自装一个插件（`enabledPlugins` 加一条）后导入 → **只**列出该插件这一条 diff，aidog 托管字段不出现。
3. 取消默认分组 / 旧无 marker 文件 → diff 行为无回归。
4. aidog 默认分组写入不覆盖 / 不删除用户自装的 `enabledPlugins`/`mcpServers` 条目。
5. `cd src-tauri && cargo build && cargo clippy`（warning 清零）+ `cargo test` 全绿；`yarn build`（tsc+vite）通过；`node scripts/check-i18n.mjs`（若有新 key）通过。

## 关键文件
- `src-tauri/src/commands/sync_settings.rs`（`write_default_claude_settings` 68-100 / `do_sync_group_settings` 134-314 / `merge_json` 106-130）
- `src-tauri/defaults/settings.json`（`enabledPlugins` 硬编码源）
- `src/pages/Settings.tsx`（224 `handleImportFromClaudeCode` / 152 `config` 加载 / 241 `applyImport`）
- `src/components/settings/editors.tsx`（2531 `buildImportDiffTree` / `ImportDiffModal`）
- `read_claude_code_settings` command（后端，`claudeSettingsImportApi.readDefault`）
- `RECOMMENDED_CONFIG` 常量（前端，fallback 不对称源）

## 约束
- 跨 Rust↔TS 边界 → 读 guides/cross-layer-rules.md，字段名/类型契约对齐。
- 写新逻辑前 grep 已有实现（code-reuse）。
- worktree 隔离实施；禁 git push。

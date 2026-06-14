# Skills 单一 skill 卸载

## 背景

Skills 页当前只有「卸载全部」（`skills_uninstall_all` → `npx skills remove --all`），缺单一 skill 卸载入口。用户无法单独移除某条 skill，只能全删重装。

## 目标

每条已装 skill 行增加「卸载」入口，调用 `npx skills remove -s <name> [-g] -y`（删规范存储 + 所有 agent symlink，对齐 `--all` 语义但限定单个 skill）。

## 交付

### 后端

1. `src-tauri/src/gateway/skills.rs`
   - 新增 `uninstall_args(name, scope) -> Vec<String>`：`["remove", "-s", <name>]` + `apply_scope` + `"-y"`。抽出便于单测（仿 `disable_args` / `uninstall_all`）。
   - 新增 `pub fn uninstall(name, scope, proxy_url) -> SkillsOpResult`：trim name 校验空 → 调 `run_npx_in_scope`。
   - 单测：`uninstall_args_global` / `uninstall_args_project`（仿 `apply_scope_global_adds_g` / `apply_scope_project_no_g`）。

2. `src-tauri/src/lib.rs`
   - 新增 `skills_uninstall(db, name, scope) -> Result<SkillsOpResult, String>` command（仿 `skills_uninstall_all`，加 `tracing::instrument` + `skills_proxy_url`）。
   - `invoke_handler!` 注册 `skills_uninstall`。

### 前端

3. `src/services/api.ts`
   - `skillsApi.uninstall(name, scope) -> invoke<SkillsOpResult>("skills_uninstall", { name, scope })`。

4. `src/pages/Skills.tsx`
   - 每行 agent 切换按钮组后增加「卸载」按钮（`btn-ghost` 小尺寸 + danger 色调，与 `btn-danger` 风格一致但小）。
   - 破坏性 → 二次确认 modal（复用现有 modal 模式：overlay + glass-elevated 卡片）。modal 内显 skill name。
   - `handleUninstall(skill)`：`setBusyKey("__uninstall_<name>__")` → `skillsApi.uninstall(skill.name, scope)` → `applyResult(res, "skills.uninstallDone")`。
   - busyKey 命名 `__uninstall_<name>__` 区分单条与「卸载全部」的 `__uninstall__`。

### i18n（8 语言 zh-CN / en-US / ar-SA / de-DE / es-ES / fr-FR / ja-JP / ru-RU）

新增 key：
- `skills.uninstall` — 按钮文案「卸载」
- `skills.uninstallConfirm` — 确认文案「将删除 skill {{name}} 及其在所有 agent 的启用配置，不可恢复。确认？」
- `skills.uninstallDone` — 成功 toast「已卸载」

## 验证

- `cargo build` + `cargo clippy --all-targets`（无 warning）
- `cargo test uninstall`（新单测通过）
- `yarn build`（tsc + vite）
- `node scripts/check-i18n.mjs`（零缺失）
- 手测：Skills 页每行显示「卸载」按钮；点击弹二次确认；确认后该 skill 从列表消失；「卸载全部」仍独立工作。

## 不做

- 不改 `uninstall_all` 语义 / UI。
- 不加批量多选卸载（仅单条）。
- 不改 scope 逻辑（沿用 global / project）。

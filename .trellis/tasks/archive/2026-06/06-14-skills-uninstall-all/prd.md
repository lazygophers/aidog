# PRD: Skills 一键卸载所有平台所有 skills

## 背景
Skills 页只能 per-skill per-agent 切换启用, 无批量卸载入口。用户需一键清空当前 scope 下所有平台(claude/codex/zed/warp/...13 agents)的全部 skills。

npx 能力: `skills remove --all` = `--skill '*' --agent '*' -y` (一行删规范存储 + 所有 agent symlink)。已验证 (`npx skills remove --help`)。

## 目标 (单交付, main worktree 内直接写)
Skills 页 header 加 "卸载全部" 按钮 (danger 样式), 二次确认 modal (禁 native confirm — 破坏 Tauri), 调后端 `skills_uninstall_all` 跑 `npx skills remove --all [-g]`。

## 范围
- `src-tauri/src/gateway/skills.rs`: 新增 `uninstall_all(scope, proxy_url) -> SkillsOpResult` (仿 `update`, args=`["remove","--all"]` + scope flag)
- `src-tauri/src/lib.rs`: 新 command `skills_uninstall_all` + 注册 invoke_handler
- `src/services/api.ts`: `skillsApi.uninstallAll(scope)`
- `src/pages/Skills.tsx`: header "卸载全部" danger 按钮 + confirm modal + handleUninstallAll (调后端 → 刷新列表 → toast)
- 8 locale json: `skills.uninstallAll` / `skills.uninstallAllConfirm` (含 {{count}}) / `skills.uninstallAllDone` / `skills.uninstalling`

## 二次确认 (破坏性, 不可逆)
- 点按钮 → modal 显示当前 scope 已装数量 + 警告不可恢复
- 二次点 "确认卸载" 才执行
- 禁用条件: `!writeReady || scopeInvalid || busyKey !== null || installed.length === 0`

## 验证
- `cargo test gateway::skills` 绿
- `cargo clippy` 无 warning
- `yarn build` exit 0
- check-i18n 零缺失 (8 语言 4 key)
- 实跑: 点卸载 → modal → 确认 → 列表清空

## 不做
- 不做 per-agent 选择卸载 (YAGNI, `--all` 已够)
- 不删规范存储目录的 fallback (npx remove --all 已删)

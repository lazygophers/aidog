# Claude Code 默认设置补通知 hook 注入 + 快捷创建

## Goal

让 aidog 默认为所有分组注入通知系统 hook（Claude Code hooks.Stop/Notification + Codex notify），通过 `_aidog_hooks` marker 驱动、在 `do_sync_group_settings` 物化（镜像 `_aidog_statusline` 机制），并在 NotificationSettings 加全局总开关快捷控制——无需逐个分组手动「一键注入」。

## 背景 / 现状缺口（已定位）

- `defaults/settings.json`（编译内置默认 CC 配置）有 `_aidog_statusline`/`_aidog_subagent_statusline` marker（默认开）但**无 hooks、无 `_aidog_hooks` marker**。
- statusLine：marker 保存时确定性物化进 baseline `claude_code` 配置（[[statusline-persistence-flow]]），sync 时 strip marker。
- hooks：仅有手动 `inject_hooks(group, client)` 命令（NotificationSettings 的 per-group「一键注入/移除」按钮，lib.rs:1899）。`do_sync_group_settings`（lib.rs:1098）只 **strip** `_aidog_hooks` marker，**不物化** → 新分组/全新安装默认不带通知 hook，必须手动点。
- 脚本路径机器相关（`~/.aidog/aidog-notify-{complete,waiting}.sh`），不能硬编码进 defaults，必须运行时生成（`generate_hook_scripts` lib.rs:1850 已存在）。

## Requirements

### R1 — `_aidog_hooks` marker 默认开
- `src-tauri/defaults/settings.json` 加 `"_aidog_hooks": { "enabled": true }`（默认开，镜像 `_aidog_statusline`）。

### R2 — sync 物化通知 hook（镜像 statusLine）
- `do_sync_group_settings`：读 base_config 的 `_aidog_hooks.enabled`，为 true 时**物化**：
  - **Claude Code**：`generate_hook_scripts()` 生成两脚本 → `inject_claude_code_hooks(&mut config, &scripts)`（hooks.rs 已有）注入每个 group config 的 `hooks.Stop`/`hooks.Notification`，**再 strip `_aidog_hooks` marker**（注入在 strip 前）。
  - **Codex**：marker enabled 时，sync 内**一次性**把 `notify=[<complete 脚本>]` 注入 `~/.codex/config.toml`（`inject_codex_notify`，hooks.rs 已有）；disabled 则 `remove_codex_notify`。（Codex notify 是全局 config.toml，非 per-group）
- enabled=false → 不注入（现有行为：仅 strip）。

### R3 — NotificationSettings 全局总开关（快捷创建）
- NotificationSettings 加总开关「默认为所有分组注入通知 hook」，控制 baseline `claude_code` 配置的 `_aidog_hooks.enabled`。
- 后端 command（如 `set_default_hooks_enabled(enabled)`）：set marker 进 baseline claude_code 配置 + re-sync（物化/移除）+ Codex 同步。
- 开 → 所有分组自动带 CC hooks + Codex notify；关 → 移除。
- **保留**现有 per-group 手动 `inject_hooks`/`remove_hooks` 按钮（细粒度覆盖/单分组操作）。
- 前端读当前 `_aidog_hooks.enabled` 显示开关态。

## Acceptance Criteria

- [ ] `defaults/settings.json` 含 `_aidog_hooks:{enabled:true}`
- [ ] sync 时 marker enabled → 每个 group settings.json 含 hooks.Stop/Notification（指向 ~/.aidog 脚本）
- [ ] sync 时 marker enabled → ~/.codex/config.toml 含 notify=[complete 脚本]；disabled → 移除
- [ ] strip marker 仍生效（settings.json 不含 `_aidog_hooks` 内部字段）
- [ ] NotificationSettings 总开关切换：开=全分组注入，关=全移除，状态正确回显
- [ ] per-group 手动按钮仍工作
- [ ] cargo clippy 无新 warning + cargo test 绿；yarn build 绿；check-i18n 零缺失；8 locale parity

## Definition of Done

- cargo clippy 无 warning + cargo test 绿；yarn build 绿；check-i18n 零缺失
- 改动落 worktree，闭环 check→commit(merge)→archive
- 更新 [[notification-module]]（默认注入机制）

## Technical Approach

- **后端** `gateway/hooks.rs`：复用 `build_hook_script`/`inject_claude_code_hooks`/`inject_codex_notify`/`remove_*`。新增/调整 sync 内物化逻辑。`MARKER_HOOKS="_aidog_hooks"` 已存在。
- **`lib.rs do_sync_group_settings`**：注入逻辑放在 strip marker **之前**；脚本生成一次（循环外），每 group 注入 CC hooks；Codex notify 循环外一次性按 marker enable/disable 注入/移除。
- **新 command** `set_default_hooks_enabled(enabled: bool)`：读 baseline claude_code（无则默认模板）→ 设 `_aidog_hooks.enabled` → 回写 DB → re-sync。
- **新 command/复用** `get_default_hooks_enabled` 或并入 notification_settings_get 回显。
- **前端** `NotificationSettings.tsx`：加总开关 + api 封装；i18n key 8 locale。
- ⚠️ 实施安全：sync 会写用户真实 `~/.aidog/settings.*.json` 与 `~/.codex/config.toml`。worktree 内 `cargo test` 用 mock/参数测试，**禁真跑 sync 改用户文件**；逻辑正确性靠单测（inject/strip/marker 解析）+ 审查。

## Out of Scope

- 通知模板/TTS/收件箱逻辑（N1 不动）
- 非 task_complete/waiting_input 的新 hook 事件
- statusLine 机制改动

## Technical Notes

- 现状：do_sync_group_settings(lib.rs:1045) strip marker(:1098) / inject_hooks(:1899) / generate_hook_scripts(:1850) / build_hook_script(hooks.rs:53) / inject_claude_code_hooks+inject_codex_notify(hooks.rs)
- defaults/settings.json 现有 `_aidog_statusline:{enabled:true}` 可作 marker 范本
- 前端 NotificationSettings.tsx handleInject(:114) per-group 按钮 + notificationApi.injectHooks/removeHooks
- 参考 [[notification-module]] / [[statusline-persistence-flow]]

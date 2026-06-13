# 通知 N2 — hook 集成

Parent: `06-13-system-notification` — 系统通知模块。共享契约见 `../06-13-system-notification/design.md`。

## Goal

在 Codex 与 Claude Code 提供一键 hook 注入：生成 hook 脚本（任务完成、等待输入）+ 自动写 Claude Code settings（Stop/Notification hook）与 Codex notify config，脚本 POST N1 的 /api/notify 端点触发通知，内置两类通知默认模板 + 变量。完成后：用户一键启用后，Claude Code/Codex 事件自动触发通知；可一键移除。

## What I already know
- 依赖 **N1**（/api/notify 端点 + 类型 + 变量替换）。
- 复用 statusline 范式：generate_statusline_script(lib.rs:1604) 生成脚本到 ~/.aidog/ + settings.{group}.json 注入 + do_sync_group_settings strip + ANTHROPIC_BASE_URL/AUTH_TOKEN。
- Codex notify 机制 + codex.rs TOML 子系统（memory codex-config-subsystem）。
- 注入/strip 细节见 parent design.md「hook 集成」。

## Deliverable 矩阵
| ID | 交付物 | 类型 | 独立验收 | 优先级 |
| --- | --- | --- | --- | --- |
| N2.1 | hook 脚本生成（complete/waiting） | diff | 脚本生成到 ~/.aidog 可执行 + POST 端点 | P0 |
| N2.2 | Claude Code settings 一键注入(Stop/Notification) + strip | diff | settings 含 hook；strip 不污染 | P0 |
| N2.3 | Codex TOML notify 注入 + 内置模板 | diff | TOML 含 notify；触发通知 | P0 |

## Requirements
- NR5 生成 Claude Code hook 脚本（Stop=任务完成、Notification=等待输入）+ Codex notify 脚本；脚本 POST /api/notify，project=cwd basename 作 vars。
- 一键注入 Claude Code settings.{group}.json（hooks.Stop/hooks.Notification）+ Codex config.toml（notify）；do_sync_group_settings strip 列表加 hook 标记防回写污染（仿 _aidog_statusline）。
- NR6 内置两类默认模板「{project} 完成」「{project} 等待用户输入」，用户可改（存 N1 NotificationSettings.per_type.template）。
- 一键移除（strip）。

## Acceptance Criteria
- [ ] 一键注入后 Claude Code settings 含 Stop/Notification hook、Codex TOML 含 notify（单测/集成）。
- [ ] hook 脚本 POST /api/notify 触发通知（手测/集成）。
- [ ] strip 逻辑不污染 group settings（仿 statusline，单测）。
- [ ] 一键移除清除 hook。
- [ ] cargo test && cargo clippy --all-targets -- -D warnings 全绿。

## Definition of Done
- Requirements 全实现 + AC 勾选；变更自动暂存提交；hook 注入/strip 范式落 cortex。

## Out of Scope
- 通知分发核心（N1）；前端 UI（N3）。

## Technical Notes
- 改 lib.rs(脚本生成 + 注入 commands)/codex.rs(Codex notify 注入)/do_sync_group_settings(strip)。
- injectHooks/removeHooks commands（命名与 N1 契约协调）。
- **必须 N1 完成后开工**（依赖端点）；与其他后端 child 全局串行（共享 lib.rs/codex.rs）。

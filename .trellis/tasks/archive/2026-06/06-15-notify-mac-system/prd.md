# macOS 通知默认走系统通知

## Goal

macOS 下 `show_popup` 当前走 `tauri-plugin-notification`，在未签名 dev build / 用户未授权时**通知不显示**或显示为 app 内 toast。改为优先 `osascript -e 'display notification ...'`：无签名要求、直进通知中心、跨 macOS 版本稳定。

## What I already know

- `notification.rs::show_popup` (L253-258) 调 `app.notification().builder().title().body().show()`。
- `tauri-plugin-notification` macOS 走 UserNotifications framework — **要求 app 已签名 + 用户授权**，dev/未签名场景常失败。
- `osascript -e 'display notification "body" with title "title"'` — macOS 原生 API、无签名、无授权弹框（首次会进入通知中心管理项），**最稳**。Codex / Claude Code 多数 macOS CLI 工具都用这条。
- 现有 `play_beep` 已 spawn `afplay` 走 std::process — 相同模式可直接复用。

## Decision (ADR-lite)

- **macOS**：`show_popup` 走 `osascript -e 'display notification "<escaped body>" with title "<escaped title>"'`，spawn 独立线程，失败仅记日志。
- **其他平台** (Linux / Windows)：保持 `tauri-plugin-notification`（Windows 走 WinRT toast，Linux 走 freedesktop notifications，都不需要额外签名）。
- **不加配置开关**：项目风格 = 默认最稳路径，不让用户选。
- **转义**：title / body 含 `"` 和 `\` 必须转义（osascript 字符串语法），避免 shell 注入。

## Requirements

- `notification.rs::show_popup` 加 `#[cfg(target_os = "macos")]` 分支：spawn `osascript`，转义 title/body。
- 其他平台路径不变。
- 新增 osascript 字符串转义 helper（pure fn，可单测）。
- 单测：`osascript_escape("\"a\\b\"")` 返回 `\"\\\"a\\\\b\\\"\"`。
- cargo clippy / test / build / i18n 全绿。

## Acceptance Criteria

- [ ] macOS dev build 任意点击「测试弹窗」🪟 钮 → 通知中心右上角出现系统通知（无 app 内 toast / 无权限弹框）。
- [ ] title/body 含特殊字符（`"`、`\`、`'`）显示正常，不爆 osascript 语法。
- [ ] 其他 OS 行为不变（仍走 tauri-plugin）。
- [ ] cargo test / clippy / yarn build / check-i18n 全绿。

## Definition of Done

- worktree commit + merge + archive
- cortex memory 落档（osascript 通知踩坑 + 转义规则）

## Out of Scope

- Linux/Windows 通知后端替换
- 通知中心 grouping / sound / action button
- 让用户选择「osascript vs tauri-plugin」配置开关

## Files

- `src-tauri/src/gateway/notification.rs` — `show_popup` macOS 分支 + 转义 helper + 单测

## Research References

无外部研究 — 内部 API 替换 + 已知系统命令。

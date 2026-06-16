# PRD — 通知 notification_test 命令无系统弹窗

## 现象

调 `notification_test`（notif_type=task_complete）日志有 `command invoked`（trace 1ebe425e / 842bcbf5），但**实际无系统弹窗**。

## 已定位路径（只读核实）

- `notification_test`（lib.rs:1953）传 `Some(&app)` 给 dispatch ✓（app 非 None，排除「无 app 跳过 popup」）
- `dispatch`（notification.rs:175）：
  - 全局 `!settings.enabled` → 旁路，返回 popup:false（:187）
  - `do_popup = ch.popup && setting.popup`（:206）；`ch = channels_for_form(setting.form)`
  - 仅 `do_popup && app.is_some()` 才调 `show_popup`（:220-229）
- macOS `show_popup`（notification.rs:259）走 `osascript -e 'display notification ... with title ...'`；失败仅 warn（:275）

## 根因假设（exec 复现验证）

**H1 — do_popup=false（到不了 osascript）**：
- task_complete 的 `setting.form` = InboxOnly → ch.popup=false；或该类型 `setting.popup` 开关关；或全局 `settings.enabled`=false
- 验证：看 `notification_test` 返回的 `DispatchResult.popup` 值（前端拿得到）+ 查 task_complete 默认 form/popup 设置
- 若属此 → 是配置/默认值问题，非弹窗机制坏；修默认或 UI 提示

**H2 — do_popup=true 但 osascript 被吞**：
- `display notification` 归属 osascript/Script Editor，若该宿主未授权通知 / DND / 专注模式 → 静默不显示
- 验证：手动跑 `osascript -e 'display notification "x" with title "y"'` 看是否弹；查日志有无 :275 warn
- 若属此 → 评估改用更可靠方式（如带 bundle id 的 terminal-notifier 思路、或 tauri-plugin 配权限、或在归属/授权上做处理）

## 验收标准

- 根因明确（H1/H2/其他），有证据（DispatchResult 返回值 / osascript 手动复现 / 日志）
- task_complete 测试触发后**确有系统弹窗**（或：若是配置默认问题，修正默认 + 说明）
- 不破坏其余通知通道（TTS/inbox/sound）与既有 osascript 转义防注入（osascript_escape :292）
- `cargo build` / `cargo clippy`（零 warning）/ `cargo test` 通过

## 资源

- src-tauri/src/gateway/notification.rs（dispatch / show_popup / channels_for_form / NotifForm）
- src-tauri/src/lib.rs:1953（notification_test 命令）
- memory: notification-macos-osascript（osascript vs tauri-plugin 取舍）/ notification-default-templates（各类型默认 form）

## 失败处理

- H1/H2 都不成立 → 扩查 channels_for_form 映射 + get_notification_settings 默认值 + 前端是否吞了 DispatchResult
- 涉 macOS 通知授权且无法在 dev 验证 → 标注 + 给可在打包版验证的路径

# 通知设置：独立 TTS / 弹窗 / 提示音测试按钮

## Goal

现有 NotificationSettings 每类型只有「测试」按钮（走完整 `dispatch`，受 type 的 form 设置约束）。补三个独立通道测试按钮，绕过 dispatch 直接触发对应通道，方便用户验证语音后端 / 弹窗权限 / 系统提示音是否工作。

## What I already know

- 现有 `notification.rs::dispatch()` 按 setting.form (Full/PopupOnly/InboxOnly/SoundOnly) 选通道，不可独立测试。
- 现有 `speak()` (L242-269) + `show_popup()` (L230-235) 是私有 fn，UI 经 dispatch 间接调用。
- sound 通道当前**无独立实现**——靠 popup 带系统声或 TTS 播报代替（notification.rs L213-215 注释）。本任务补一个 `play_beep()`。
- 设置流：`get_notification_settings(db)` → settings.tts_backend / tts_enabled。

## Decision (ADR-lite)

**Context**：用户选「每个 type 按钮旁加两个快捷」+ 提示音独立测试。每类型行：`[测试][🔊][🪟][🔔]`，4 钮一排（6 type × 4 = 24 钮过多；只对 4 类型生效 = 4×4=16 钮）。

**Decision**：
- **后端 3 个新命令**：`notify_test_tts(text)` / `notify_test_popup(title, body)` / `notify_test_beep()`，绕过 dispatch 直接调对应通道。
  - TTS 从 settings 取 backend (CrossPlatform/MacSay/WebSpeech)。
  - popup 直接调 `tauri_plugin_notification`。
  - beep 跨平台 spawn：macOS=`afplay /System/Library/Sounds/Pop.aiff`、Windows=`powershell [console]::Beep`、Linux=`paplay <freedesktop bell>` fallback `printf '\a'`。
- **notification.rs** 将 `speak`/`show_popup` 内部 fn 暴露为 `pub(super)` 或抽公开包装；新增 `play_beep()` pub。
- **前端**：每类型行 4 钮 = 现有「测试」+ 3 个图标按钮 (🔊 = TTS, 🪟 = popup, 🔔 = beep)。点击调对应 API。文案带 i18n key。
- **测试参数**：测试时用 type label + `测试` 后缀作为 title/body 文本（无 project 也可识别）。

**Consequences**：
- 后端命令数 +3。
- UI 4 type × 4 钮 = 16 钮（紧凑用 icon button + tooltip）。
- 跨平台 beep 用 `std::process::Command` spawn 现有系统命令，0 额外依赖。

## Requirements

- 后端 `gateway::notification` 暴露 `speak_text(app, backend, text)` + `show_popup_at(app, title, body)` + 新增 `play_beep()`。
- 后端 `lib.rs` 3 新 command：`notify_test_tts` / `notify_test_popup` / `notify_test_beep`，注册到 `generate_handler!`。
- 前端 `api.ts` `notificationApi` 加 3 invoke wrapper。
- 前端 `NotificationSettings.tsx` 每 type 行 4 钮 + i18n key × 8 locale。
- 测试：notification 模块单测加 `play_beep` smoke（仅检查 spawn 不 panic；macOS 路径存在性 sanity）。
- cargo clippy / test / yarn build / check-i18n 全绿。

## Acceptance Criteria

- [ ] 4 类型行各有 4 钮 [测试 / 🔊 / 🪟 / 🔔]，hover 显示 tooltip。
- [ ] 🔊 仅触发 TTS（按当前 backend 播报「<type 名> 测试」）。
- [ ] 🪟 仅触发系统弹窗（title=type 名，body=「测试通知」）。
- [ ] 🔔 仅触发系统提示音（macOS Pop.aiff / Windows beep / Linux bell）。
- [ ] 不动设置 / 不落 inbox / 不走 dispatch。
- [ ] cargo / clippy / build / i18n 全绿。

## Definition of Done

- worktree commit + merge + archive
- cortex memory 落档

## Out of Scope

- 改 dispatch 通道选择逻辑
- beep 配置音色 / 音量
- 测试结果反馈到前端 toast（直接调用即可）

## Files

- `src-tauri/src/gateway/notification.rs` — 抽公开包装 + `play_beep()` + smoke 测
- `src-tauri/src/lib.rs` — 3 commands + generate_handler!
- `src/services/api.ts` — notificationApi 3 wrapper
- `src/components/settings/NotificationSettings.tsx` — 每 type 行 4 钮
- `src/locales/*.json` (×8) — 3 i18n key

## Research References

无外部研究 — 内部 API 扩展 + 跨平台命令清单 (已知)。

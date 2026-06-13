# 通知 N3 — 前端

Parent: `06-13-system-notification` — 系统通知模块。共享契约见 `../06-13-system-notification/design.md`。

## Goal

前端落地：通知设置 UI（按类型 播报/弹窗/呈现形式 + TTS 开关与后端选择）+ 应用内通知中心（收件箱：历史/未读/清除）+ Codex&Claude Code 一键注入入口 + WebSpeech 播报（消费 N1 emit 的 tts 事件）+ 7 语言 i18n。完成后：设置可配、通知中心展示、一键注入可用、7 语言无缺键。

## What I already know
- 依赖 **N1 契约**（notificationApi + 类型）+ N2 的 injectHooks/removeHooks command。
- 与 C5/GB 前端同碰 AppSettings/api.ts/i18n → 前端串行。
- 前端约定 spec/frontend/conventions.md；Liquid Glass；i18n 7 语言 ar-SA RTL；无 react-router（导航本地 state，新页加侧栏入口或 AppSettings tab）。

## Deliverable 矩阵
| ID | 交付物 | 类型 | 独立验收 | 优先级 |
| --- | --- | --- | --- | --- |
| N3.1 | 通知设置 UI（按类型 + TTS 后端 + 模板编辑） | UI | 可配置往返 | P0 |
| N3.2 | 通知中心（收件箱）页 + 未读 | UI | 展示/标读/清除 | P0 |
| N3.3 | 一键注入入口 + WebSpeech 播报 + i18n 7 语言 | UI/diff | 注入按钮；webspeech 播报；无缺键 RTL | P0 |

## Requirements
- NR7 设置 UI：总开关 + tts_enabled + tts_backend 选择 + 按类型 {tts,popup,form,template} 编辑 + 变量提示。
- 通知中心页：历史列表 + 未读计数 + 标记已读 + 清除（侧栏入口或 AppSettings 区，依现有导航模式）。
- 一键注入入口：调 injectHooks/removeHooks（按 group/client 选 Claude Code/Codex）。
- WebSpeech：tts_backend=WebSpeech 时监听 N1 emit 的 tts 事件，用 Web Speech API 播报。
- i18n 7 语言全覆盖，ar-SA RTL 正常。

## Acceptance Criteria
- [ ] yarn build（tsc && vite build）通过。
- [ ] 设置页按类型可配 + TTS 后端选择 + 模板编辑；往返保存。
- [ ] 通知中心展示历史/未读/标读/清除。
- [ ] 一键注入入口调用 command；WebSpeech 后端可播报。
- [ ] 7 语言无缺键；ar-SA RTL 正常。

## Definition of Done
- Requirements 全实现 + AC 勾选；变更自动暂存提交。

## Out of Scope
- 后端分发/端点（N1）；hook 脚本生成/注入逻辑（N2，前端仅调 command）；独立音量。

## Technical Notes
- 改 src/pages/AppSettings.tsx(通知设置 tab)、新增通知中心页 + 侧栏入口(App.tsx)、src/services/api.ts(消费 N1 契约)、i18n 资源。
- WebSpeech 监听 Tauri event（N1 emit）。
- **前端串行**：开工前合入含 N1 契约的最新 master + 确认 C5/GB 前端改动已合（避免 AppSettings/api.ts/App.tsx 冲突）。
- 只消费契约，不改后端。

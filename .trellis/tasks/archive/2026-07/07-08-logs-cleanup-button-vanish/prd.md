# 清理日志按钮随记录开关消失 — 结构性修复

## Goal

「清理过期」/「清除全部」按钮被包在 `{logEnabled && (...)}` 条件块内（`LogSettingsSection.tsx:84` 开启，`:209` 闭合，按钮位于 `:175-194`）。用户关掉「记录请求日志」master 开关后，整块不渲染，清理按钮跟着消失 —— 关了记录反而锁死清理入口，已存日志无法清理，DB 持续膨胀。

这是前 4 轮 logs-cleanup 任务（buttons / location / feedback-toast / loading）遗留的结构性 bug：一直在改反馈样式（toast/loading/着色），没人发现按钮挂错条件块。用户反复反馈「按钮消失了 / 没反馈」的根因正在此。

## Root Cause

`src/pages/AppSettings/LogSettingsSection.tsx`:
- `:84` `{logEnabled && (` 开启条件块（含 retention 天数配置 + 清理按钮）
- `:175-194` 清理过期 + 清除全部按钮
- `:209` `)}` 闭合

**语义错位**：`logEnabled`（master switch）= 是否记录**新**日志；清理/清除 = 操作**已存**日志。两者独立，关闭记录不该隐藏清理入口。即使不再写新日志，已落库的 proxy_log 行仍需可清。

## Requirements

1. 清理过期 + 清除全部按钮**始终可见可点**，不依赖 `logEnabled`
2. retention 天数配置（userReqRetention / upstreamReqRetention / logRetention 输入框）的可见性**保持现状**（仍受 logEnabled 控制，或独立 —— 见 Decision）
3. 按钮现有交互保留：busy 态文案「清理中...」+ disabled + toast 反馈 + createPortal 确认弹窗
4. logRetention=0 时「清理过期」仍 disabled（永久保留模式无过期日志），title 保留 hint

## Decision (ADR-lite)

**Context**: 按钮挂在 logEnabled 块内导致开关关闭后消失。
**Decision**: 把清理按钮块（`:175-209` 的 `<div>` 容器）**移出** `{logEnabled && ...)}` 条件块，作为同级独立 section 始终渲染。retention 天数输入框**保留在 logEnabled 块内**（未启用记录时配 retention 无意义，避免误导）。
**Consequences**:
- 关闭记录开关后，retention 配置隐藏但清理按钮仍在 —— 用户可清已存日志
- 视觉上清理 section 独立成块，与 retention 配置分离，语义更清晰
- 需调整外层容器/wrapper 确保布局不破

## Acceptance Criteria

- [ ] logEnabled=false 时「清理过期」「清除全部」按钮仍渲染且可点
- [ ] logEnabled=true 时按钮行为与现状一致（busy/disabled/toast/弹窗）
- [ ] logRetention=0 时「清理过期」disabled + hint title 保留
- [ ] 点击「清理过期」触发 `proxyLogApi.cleanupExpired()` + toast 成功/失败反馈
- [ ] 点击「清除全部」弹 createPortal 确认弹窗，确认后触发 `proxyLogApi.clear()`
- [ ] retention 输入框仍受 logEnabled 控制（开关关 → 输入框隐藏）
- [ ] `yarn build`（tsc + vite）通过

## Out of Scope

- 不改后端（proxy_log_cleanup_expired / proxy_log_clear 命令正确）
- 不改 Logs 列表页（Logs/ListView.tsx）的同名按钮（双重入口，已归档 task 决定保留）
- 不改 toast 样式 / loading 交互（上轮 task 已做）
- 不改 retention 配置的可见性逻辑（保持现状受 logEnabled 控制）

## Technical Notes

- 文件：`src/pages/AppSettings/LogSettingsSection.tsx`（329 行）
- 关键行：`:84` 条件块开 / `:175-194` 按钮容器 / `:209` 条件块闭
- 前序 commit：`95eb8591`（loading + toast 着色）/ `c624b717`（迁设置页）/ `7fa81e9a`（拆 AppSettings 巨石）
- i18n key 已齐（logs.cleanupExpired / logs.clear / logs.cleaning / logs.cleanupDisabledHint / logs.clearConfirm / logs.clearDone / logs.cleanupExpiredDone），无新增

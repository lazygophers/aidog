# 日志清理按钮位置修正 + 点击反馈

## Goal
上 task（07-08-logs-cleanup-buttons）把「清理过期」「清空」按钮加到了 Logs 列表页工具栏。用户验收发现：
1. **位置错**：用户期望按钮在**设置页 → 记录请求日志 section**（`LogSettingsSection.tsx`，`proxy.logRequests`="记录请求日志"），紧邻保留天数配置（语义最相关）。非 Logs 列表页。
2. **点击无反馈**：「清理过期」点击后 cleanupMessage state 在 `useLogsData`（Logs 页 hook），设置页看不到，用户点了没任何提示。

本 task 修正：在 `LogSettingsSection.tsx` retention 配置区底部加「清理过期」+「清空全部」两按钮，带可见点击反馈 + 清空确认 modal。

## Background / 已知
- 后端契约已就绪（上 task 已 merge 进 master）：
  - `proxy_log_cleanup_expired` command（无参，返 `Result<(), String>`，按当前 settings 清理，不写设置）
  - `proxy_log_clear` command（软删全清）
- 前端封装已就绪：`src/services/api/proxy.ts` `proxyLogApi.cleanupExpired(): Promise<void>` + `proxyLogApi.clear(): Promise<void>`
- 设置页结构：`src/pages/AppSettings/LogSettingsSection.tsx`
  - 纯展示组件，props `{ s: SystemSettings }`
  - retention 配置区在 `logEnabled &&` 块内（line ~95-133），三个 input（userReq/upstreamReq/logRetention）
  - 组件末尾 line ~137 `{logEnabled && (...)}` 块**结束后**、section `</div>` 前 —— 按钮加在 retention 区底部（retention 配置后，仍在 `logEnabled` 块内）
- createPortal modal 模板：`src/components/settings/MitmConfig.tsx:582`（showClearConfirm state + fixed inset 0 + glass-surface + 取消/确认 btn）
- Logs 列表页已有按钮（上 task）——**保留不动**（双入口合理：列表页快速操作 + 设置页配置语义）
- i18n key 已有（上 task merge）：`logs.cleanupExpired` / `logs.cleanupExpiredDone` / `logs.clearConfirmTitle` / `logs.clearConfirm` / `logs.cancel` / `logs.clear`

## Requirements

### LogSettingsSection.tsx 改动
1. 加本地 state（组件内 `useState`，不需扩 useSystemSettings）：
   - `showClearConfirm: boolean`（清空确认 modal）
   - `busy: boolean`（操作进行中，禁按钮）
   - `message: string`（反馈文案，成功/失败 inline 显示，3 秒后自清）
2. handler：
   - `handleCleanupExpired`：`setBusy(true)` → `await proxyLogApi.cleanupExpired()` → `setMessage(t("logs.cleanupExpiredDone"))` → 3 秒清 → `setBusy(false)`；catch → `setMessage(错误)`
   - `handleClearAll`（确认后）：`setBusy(true)` → `await proxyLogApi.clear()` → `setMessage(t("logs.clearDone","已清空"))` → 关 modal → `setBusy(false)`
3. UI：retention 区底部（line ~133 `</div>` retention 区结束后、`</>` logEnabled 块结束前）加一行按钮：
   ```
   <div style={{ display: "flex", gap: 8, paddingTop: 8, borderTop: "1px solid var(--border)" }}>
     <button className="btn" onClick={handleCleanupExpired} disabled={busy || logRetention===0}>
       {t("logs.cleanupExpired","清理过期")}
     </button>
     <button className="btn btn-danger" onClick={() => setShowClearConfirm(true)} disabled={busy}>
       {t("logs.clear","清除全部")}
     </button>
     {message && <span style={{ fontSize: 11, color: "var(--text-tertiary)", alignSelf: "center" }}>{message}</span>}
   </div>
   ```
   - 「清理过期」当 `logRetention===0`（永久保留）时 disabled（无过期概念）
4. 清空确认 modal：组件末尾 `{showClearConfirm && createPortal(..., document.body)}`，复用 MitmConfig.tsx:582 结构（fixed inset 0 + glass-surface + 标题 `logs.clearConfirmTitle` + 正文 `logs.clearConfirm` + 取消 `logs.cancel` / 确认 btn-danger）

### i18n
- `logs.clearDone`（"已清空"）8 语言补（其余 key 已有）
- 若 `proxy.*` 命名空间更合适则用 `proxy.cleanupExpired` 等（检查现有 logs.* key 是否 Logs 页专用；若设置页用 proxy.* 更一致则迁移）—— 实际 logs.* 已 merge，设置页复用无歧义，保留 logs.*

### Logs 列表页（ListView.tsx）
- 保留现有按钮（上 task）不动
- 但「清理过期」反馈 cleanupMessage 仅列表页可见 —— 可接受（列表页操作反馈在列表页）。设置页用独立 message state。

## Acceptance Criteria
- [ ] 设置页「记录请求日志」section retention 区底部显示两按钮
- [ ] 「清理过期」点击 → 调 API → inline message 显示「已清理过期日志」3 秒
- [ ] `logRetention===0` 时「清理过期」disabled（永久保留无过期）
- [ ] 「清除全部」点击 → createPortal modal 确认 → 确认后清空 + message；取消关闭
- [ ] modal 在 liquid glass 主题窗口居中（portal document.body）
- [ ] `cargo clippy`/`cargo test` 无回归（本 task 不动 Rust）
- [ ] `yarn build` 0 错误
- [ ] 8 locale 含 `logs.clearDone`（若新增）
- [ ] `node scripts/check-i18n.mjs` 绿

## Out of Scope
- 不动后端（上 task 已就绪）
- 不动 Logs 列表页按钮（保留双入口）
- 不改 cleanup 算法

## Technical Notes
- 跨层契约已对齐（上 task），本 task 纯前端 UI
- modal 规则见 CLAUDE.md + memory `modal-window-center-rule`
- 复用 MitmConfig.tsx:582 modal 模板

# 请求日志一键清理/清空按钮

## Goal

请求日志页（Logs）当前只有"清除全部"按钮，且该按钮用原生 `confirm()`（违反 CLAUDE.md「禁原生 confirm / beforeunload，破坏 Tauri」+「modal 必须 createPortal(document.body)」）。本任务交付两个功能：

1. **一键清理过期**（新）：按用户已设的保留天数（`ProxyLogSettings` 的 user/upstream/retention 三级）立即触发清理，不改设置。
2. **一键清空**（修复）：保留现有软删全清行为，但把原生 `confirm()` 换成 `createPortal(document.body)` 确认弹窗。

## Background / 已知（auto-context 探明）

- 后端清理链已存在（`proxy_log_settings_set` 调用，commands/proxy_log.rs:115-145）：
  - `cleanup_user_request_fields(db, user_request_retention_days)`（maintenance.rs:127）
  - `cleanup_upstream_request_fields(db, upstream_request_retention_days)`（maintenance.rs:154）
  - `cleanup_proxy_logs(db, retention_days)`（proxy_log.rs:489，`DELETE WHERE created_at < ? AND deleted_at = 0`）
  - `purge_deleted_proxy_logs(db)`（proxy_log.rs:515，清软删 tombstone）
- 现有 `proxy_log_clear` command（proxy_log.rs:56）→ `clear_proxy_logs`（软删全清，test_proxy_log.rs:335）
- 前端封装 `proxyLogApi`（src/services/api/proxy.ts:35），已有 `clear()` / `count()` / `settings.get/set`
- Logs 页结构：Logs.tsx 外壳 → `Logs/{useLogsData, ListView, DetailPanel, primitives}`
  - `useLogsData.ts:174` `handleClear` 用原生 `confirm(t("logs.clearConfirm"))`（**违规**）
  - `ListView.tsx:41` 工具栏按钮 `btn-danger`，仅 `total > 0` 时显示
- 现成 createPortal confirm modal 模板：`MitmConfig.tsx:582` `showClearConfirm`（portal body + glass-surface + 取消/确认按钮，禁 window.confirm 注释明确）
- handler 注册点：`startup.rs:103`（`proxy_log_clear` 邻位）
- settings_get 能读当前设置（含三级 retention_days）→ cleanup_expired command 无需前端传参，后端自取

## Requirements

### 后端（Rust）

- **新 command `proxy_log_cleanup_expired`**（commands/proxy_log.rs）：
  - 读当前 `ProxyLogSettings`（同 `proxy_log_settings_get` 逻辑：`get_setting(db,"proxy","logging")` → unwrap_or_default）
  - 跑清理链（复用 settings_set 的 4 步序列，**不写设置**）：user/upstream fields + retention_days(>0 时) + purge tombstone
  - 每步 `tracing::warn!` 容错（同 settings_set 模式，单步失败不阻塞其余）
  - 返回 `Result<u64, String>`：清理掉的 tombstone / 过期行数（purge_deleted_proxy_logs 返回影响行数；如不易取，返 `()` 亦可，前端不强依赖数字）
- startup.rs 注册新 command（proxy_log_clear 邻位）

### 前端（React/TS）

- `proxyLogApi.cleanupExpired(): Promise<number>`（api/proxy.ts，invoke `proxy_log_cleanup_expired`）
- `useLogsData.ts`：
  - 新 `handleCleanupExpired`：调 `proxyLogApi.cleanupExpired()` → 成功后 `setOffset(0); load()` + 可选 toast/message 反馈；失败 console.error
  - **修复 `handleClear`**：移除原生 `confirm()`，改用 state 控制的 createPortal modal（state 提升到 useLogsData，导出 `showClearConfirm` + `setShowClearConfirm` 给 ListView 渲染；或 modal 组件直接在 useLogsData 渲染）
- `ListView.tsx` 工具栏：
  - 现有"清除全部"按钮（btn-danger）保留
  - 新增"清理过期"按钮（普通 btn 样式，非 danger，按保留天数清理语义非破坏性），仅 `total > 0` 时显示
  - 渲染清空确认 modal（createPortal body，复用 MitmConfig 模式）

### i18n（8 语言）

新增 key（zh-Hans / en-US / ar-SA / fr-FR / de-DE / ru-RU / ja-JP / es-ES）：
- `logs.cleanupExpired` — "清理过期"按钮文案
- `logs.cleanupExpiredDone` — 清理完成反馈（带 `{{n}}` 行数插值，或简单"已清理过期日志"）
- `logs.clearConfirmTitle` — 清空确认弹窗标题（"清空全部日志"）
- 现有 `logs.clearConfirm` 文案保留作弹窗正文
- `logs.cancel` — "取消"（如 logs 命名空间无此 key 则补；已有则复用）
- `logs.clear` — "清除全部"（已有）

## Acceptance Criteria

- [ ] `cargo clippy` 0 warning，`cargo test` 全过（含 proxy_log / maintenance 既有测试）
- [ ] `yarn build`（tsc && vite build）0 错误
- [ ] 新 command `proxy_log_cleanup_expired` 在 startup.rs 注册，前端可调通
- [ ] "清理过期"按钮点击后日志列表刷新（过期行消失）
- [ ] "清除全部"按钮点击弹 createPortal modal（非原生 confirm），确认后软删全清 + 列表刷新；取消则关闭不动
- [ ] modal 在 liquid glass 主题下窗口居中（portal body，非 page 内）
- [ ] `grep -rn "confirm(" src/pages/Logs/` = 0（无原生 confirm 残留）
- [ ] 8 locale 文件均含新增 key，`scripts/check-i18n.mjs` 绿（若项目有此脚本）
- [ ] 阿拉伯语 ar-SA RTL 下按钮 + modal 布局正常

## Definition of Done

- 上述 Acceptance Criteria 全过
- 跨 Rust↔TS 边界字段对齐（command 名 / 参数 / 返回类型三侧一致，参考 `.trellis/spec/guides/cross-layer-rules.md`）
- 无新增 lint/type 错误

## Out of Scope

- 不改保留天数设置 UI（仅消费现有 settings）
- 不改 cleanup 算法本身（仅抽序列复用）
- 不加"清理进度条"（同步操作，量小）
- 不动 proxy_log_clear 的软删语义（保持现状）
- 不加自动定时清理（已有 settings_set 触发 + 启动时清理，足够）

## Technical Approach

**后端**：把 `proxy_log_settings_set`（proxy_log.rs:115-145）的清理链抽成 `pub async fn run_retention_cleanup(db, &settings)` 内部函数（或直接在新 command 内联复用），新 command `proxy_log_cleanup_expired` 读 settings → 调该函数。避免代码重复（遵守 `.trellis/spec/guides/code-reuse-rules.md`）。

**前端 modal**：state + handler 放 useLogsData（与其他 handler 同位），modal JSX 在 ListView 渲染（因 ListView 已是展示层 + 持有工具栏）。复用 MitmConfig.tsx:582 的 portal + glass-surface + 取消/确认按钮结构。

## Decision (ADR-lite)

**Context**：清理过期是否需确认 modal？
**Decision**：不要 modal，直接执行 + 反馈。"按保留天数清理过期"= 执行用户已设保留策略，语义非破坏（符合用户预期保留窗口）；与 settings_set 保存时自动清理行为一致（那个也不弹 modal）。清空则必须 modal（破坏全删）。
**Consequences**：UX 一致性（按策略清 vs 全删清分开），减少弹窗疲劳。

## Technical Notes

- 跨层契约参考 `.trellis/spec/guides/cross-layer-rules.md`
- 前端约定参考 `.trellis/spec/frontend/conventions.md`（组件/state/API/i18n）
- modal portal 规则见项目 CLAUDE.md「UI / i18n」段 + memory `modal-window-center-rule`
- 复用检查参考 `.trellis/spec/guides/code-reuse-rules.md`

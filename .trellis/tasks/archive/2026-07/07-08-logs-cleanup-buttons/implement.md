# Implementation Plan — logs-cleanup-buttons

## 调度分析

**单 task，单 worktree，单 subtask 串行执行**。

理由：跨 Rust↔TS 边界契约一致性要求高（command 名/参数/返回类型三侧对齐），单 agent 一次性改保证决策一致（遵守 `.trellis/spec/guides/cross-layer-rules.md`）。改动面 <200 行，无需 fan-out 并行。

## 文件集（write-files）

| 层 | 文件 | 改动 |
|---|---|---|
| Rust | `src-tauri/src/commands/proxy_log.rs` | 新 `proxy_log_cleanup_expired` command + 抽 `run_retention_cleanup` 内部函数（复用 settings_set 清理链） |
| Rust | `src-tauri/src/startup.rs` | 注册新 command（proxy_log_clear 邻位，line ~103） |
| TS | `src/services/api/proxy.ts` | `proxyLogApi.cleanupExpired()` 封装（line ~42 邻位） |
| TS | `src/pages/Logs/useLogsData.ts` | 新 `handleCleanupExpired` + `showClearConfirm` state；修复 `handleClear` 移除原生 confirm；导出新字段 |
| TS | `src/pages/Logs/ListView.tsx` | 工具栏加"清理过期"按钮；渲染清空确认 createPortal modal |
| i18n ×8 | `src/locales/{zh-Hans,en-US,ar-SA,fr-FR,de-DE,ru-RU,ja-JP,es-ES}.json` | 新 key：`logs.cleanupExpired` / `logs.cleanupExpiredDone` / `logs.clearConfirmTitle` / `logs.cancel`（若缺） |

## 执行步骤（单 subtask，顺序内）

1. **后端先行**（契约源）：
   - `commands/proxy_log.rs`：抽 `pub(crate) async fn run_retention_cleanup(db: &Db, settings: &ProxyLogSettings) -> Result<u64, String>`（把 settings_set 的 4 步清理链搬入，返回最后一步 purge 影响行数；每步 warn 容错）
   - `proxy_log_settings_set` 改调 `run_retention_cleanup`（去重，保行为不变）
   - 新 `#[tauri::command] proxy_log_cleanup_expired(db)`：读 settings（同 settings_get）→ `run_retention_cleanup` → 返回行数
   - `startup.rs` 注册
2. **前端 API**：`proxyLogApi.cleanupExpired(): Promise<number>`
3. **前端逻辑**（useLogsData）：
   - 加 `const [showClearConfirm, setShowClearConfirm] = useState(false)`
   - `handleClear` 改为：`setShowClearConfirm(true)`（开 modal），实际清空逻辑移到 `confirmClearAll`（modal 确认按钮调）
   - 新 `handleCleanupExpired`：`await proxyLogApi.cleanupExpired()` → 刷新（`setOffset(0); load()`）+ 设 `cleanupMessage` 反馈（可选 toast）
   - 导出 `showClearConfirm` / `setShowClearConfirm` / `confirmClearAll` / `handleCleanupExpired` / `cleanupMessage`
4. **前端 UI**（ListView）：
   - `LogsData` 类型（useLogsData 末尾的 `export interface LogsData`）扩字段
   - 工具栏 `total > 0` 块内，"清除全部"前加"清理过期" `<button className="btn">`
   - 组件末尾渲染 `{showClearConfirm && createPortal(...)}`（复用 MitmConfig.tsx:582 结构：fixed inset 0 + glass-surface + 标题/正文/取消/确认 btn-danger）
5. **i18n**：8 文件加 key（zh-Hans 先写，其余 7 语言翻译；阿拉伯语 RTL 文案注意）

## 验收门禁（agent 自跑）

```bash
cd src-tauri && cargo clippy --all-targets -- -D warnings && cargo test
yarn build
grep -rn "confirm(" src/pages/Logs/   # 期望 0
node scripts/check-i18n.mjs 2>/dev/null || true   # 若存在则必须绿
```

## 失败处理

- `run_retention_cleanup` 抽函数若致 settings_set 行为漂移 → 回退为内联两份（优先保行为一致，去重次之）
- modal 在某主题下不居中 → 检查是否真 portal 到 document.body（非 page 内子节点）
- 返回行数类型不一致（Rust u64 vs TS number）→ Tauri serde 自动处理，TS 端 `Promise<number>`

## 资源

- Active task: `.trellis/tasks/07-08-logs-cleanup-buttons`
- spec: `.trellis/spec/guides/{cross-layer-rules,code-reuse-rules}.md` + `.trellis/spec/frontend/conventions.md`
- 模板：`src/components/settings/MitmConfig.tsx:582`（createPortal confirm modal）

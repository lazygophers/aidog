# 实施计划 — realtime-event-notify

决策见 `prd.md`（方案 c，scope = Logs + popover）。先读 prd.md + research/frontend-refresh-mechanism.md 再动手。

## S0 — popover 跨 webview 可达性验证（首步，定 S3 改造 vs 降级）

1. Read `src-tauri/src/commands/popover.rs` + `src-tauri/src/startup.rs`，grep `WebviewWindow` / `WindowBuilder` / popover window label，确认 popover 是独立 webview window（非 native NSPopover）
2. 确认 Tauri 2.x `app.emit(event, payload)` 广播所有 webview（`AppHandle::emit` vs `emit_to`；可 WebFetch Tauri 2.x 事件文档佐证）
3. 结论：
   - **可达**（独立 webview + emit 广播）→ S3 走事件订阅
   - **不可达** → S3 降级 `document.addEventListener("visibilitychange", reloadPopover)` + `usePolling(reloadPopover, 5_000)` 兜底

> 已知线索：`popover.tsx:3` 用 `getCurrentWindow`（`@tauri-apps/api/window`）+ import `services/api`（含 `invoke`/`groupApi`）→ 独立 webview runtime，`listen` 同 runtime 可用。推测可达。

## S1 — Logs 列表事件订阅

`src/pages/Logs.tsx:193`：

- `usePolling(refreshList, 3000, !detail)` → 拉长到 `30_000`（兜底防事件丢失 + 流式收敛）
- 加 `useEffect(() => onProxyLogUpdated(() => refreshList(), 500), [refreshList])`
- import `onProxyLogUpdated` from `services/api`（确认已 export，`api.ts:1574`）

## S2 — Logs 详情事件订阅

`src/pages/Logs.tsx:236`：

- `usePolling(refreshDetail, 2000, !!detail)` → 拉长到 `5_000`（兜底）
- 加 `useEffect(() => onProxyLogUpdated(() => refreshDetail(), 1000), [refreshDetail])`
- debounce 1000ms（流式单条 log 多次 emit，避免高频 reload 详情）

## S3 — popover 浮窗事件订阅（依 S0 结论）

`src/popover.tsx` mount useEffect（现有 fetch 逻辑旁）：

**S0 可达**：
```ts
useEffect(() => onProxyLogUpdated(() => {
  reloadPopover();  // 复用 mount fetch：popover_data + collectStatsQueries
}, 1000), []);
```

**S0 不可达（降级）**：
```ts
useEffect(() => {
  const handler = () => { if (!document.hidden) reloadPopover(); };
  document.addEventListener("visibilitychange", handler);
  return () => document.removeEventListener("visibilitychange", handler);
}, []);
// + usePolling(reloadPopover, 5_000)  // 兜底
```

- 抽 `reloadPopover` 函数（现有 mount fetch 逻辑提取复用，避免重复）
- import `onProxyLogUpdated` from `./services/api`

## 测试

- 现有 `src/services/api.test.ts:106-122` 覆盖 `onProxyLogUpdated` debounce（不改封装，应仍绿）
- Logs / popover 改动是 useEffect 接线，无新纯函数可测；靠手动 + build 验证
- 若 `reloadPopover` 抽出后可加单测（可选）

## 验收 (全过才算完)

1. S0 结论明确（可达 / 降级），记录到 prd 或返回摘要
2. S1/S2 Logs 订阅接线 + 兜底轮询拉长
3. S3 popover 订阅（或降级）接线
4. `cargo test` + `cargo clippy --all-targets -- -D warnings` + `yarn build` + `check-i18n.mjs` 全绿
5. 无新 warning

## 执行顺序

单 agent 顺序 S0 → S1 → S2 → S3（S3 依赖 S0 结论）。完成后 main 跑 check。

## 禁

- 禁改后端 `log.rs` emit 逻辑
- 禁改 `onProxyLogUpdated` 封装默认 debounce
- 禁碰 PopoverConfigTab / TrayConfigTab 30s 轮询
- 禁 git commit / push

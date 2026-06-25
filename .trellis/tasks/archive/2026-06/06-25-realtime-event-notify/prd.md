# 前端实时数据事件通知替代轮询

## 需求 (用户)

浮窗 / 首页 / 弹窗 / 日志 / 统计等各处实时数据更新，从「轮询 / mount fetch 读 DB」改为「后端事件通知 → 前端监听」，减少无谓 DB 查询 + 提升实时性。

## 现状 (research/frontend-refresh-mechanism.md)

**aidog 已大部分走事件通知**：

- 后端 `upsert_log` 写库后 `app.emit("proxy-log-updated", platform_id)`（`proxy/log.rs:153`），高频（每请求）
- 前端封装 `onProxyLogUpdated(cb, debounceMs=500)`（`services/api.ts:1574-1590`），500ms debounce 聚合高频 emit
- **4 页已订阅**：Home / Platforms / Groups / Stats

**剩余 2 处非事件化缺口**：

| 场景 | 现状 | 问题 |
| --- | --- | --- |
| Logs 列表 (`Logs.tsx:193`) | `usePolling(refreshList, 3000ms)` | 空闲也每 3s 查 DB；高峰 3s 延迟 |
| Logs 详情 (`Logs.tsx:236`) | `usePolling(refreshDetail, 2000ms)` | 同上，等流式聚合终态 |
| popover 浮窗 (`popover.tsx:91-120`) | mount fetch 一次 | 开窗期间数据冻结，请求进来不更新 |

**保持现状（合理）**：PopoverConfigTab/TrayConfigTab 30s 配置预览轮询（编辑期不应打断）；CRUD 低频不 emit（前端写后主动 reload + DOM 广播）。

## 已定决策 (用户裁定)

**方案 (c) 混合 — 改造范围 = Logs + popover**（research 推荐，用户选定）：

1. **Logs 列表 + 详情**：加 `onProxyLogUpdated` 订阅，保留长间隔兜底轮询（防事件丢失 + 流式收敛）
2. **popover 浮窗**：mount useEffect 加 `onProxyLogUpdated` 订阅
3. **popover 跨 webview 可达性**：popover 是独立 Tauri webview window（`getCurrentWindow`，`popover.tsx:3`），Tauri 2.x `app.emit` 默认广播所有 webview → **推测可达**；exec 首步验证，不可达则降级 `visibilitychange` + 5s 兜底轮询
4. **不改**：30s 配置预览轮询、CRUD emit、后端 emit 节流（前端 debounce 已够）

## 改造点

### S1 — Logs 列表事件订阅 (`src/pages/Logs.tsx:193`)

```ts
// 现状
usePolling(refreshList, 3000, !detail);
// 改为
usePolling(refreshList, 30_000, !detail);  // 拉长兜底（防事件丢失）
useEffect(() => onProxyLogUpdated(() => refreshList(), 500), [refreshList]);
```

- 空闲页（可见无请求）：0 IPC（事件不来就不刷）
- 高峰：500ms debounce 后实时刷新

### S2 — Logs 详情事件订阅 (`src/pages/Logs.tsx:236`)

```ts
// 现状
usePolling(refreshDetail, 2000, !!detail);
// 改为
usePolling(refreshDetail, 5_000, !!detail);  // 兜底防流式事件丢失
useEffect(() => onProxyLogUpdated(() => refreshDetail(), 1000), [refreshDetail]);
```

- 详情 debounce 1000ms（流式单条 log 多次 emit，避免高频 reload）
- payload 带 platform_id，但单条 log 多次写 → 直接全量 reload 详情（流式结束即稳定）

### S3 — popover 浮窗事件订阅 (`src/popover.tsx`)

mount useEffect（现有 fetch 逻辑旁）加：

```ts
useEffect(() => onProxyLogUpdated(() => {
  // 重拉 popover_data + collectStatsQueries（复用现有 mount fetch 逻辑）
  reloadPopover();
}, 1000), []);
```

- debounce 1000ms（浮窗卡片多，避免高频 re-render）
- **前置 S0 验证**跨 webview 可达性

### S0 — popover 跨 webview 可达性验证（exec 首步）

- 读 `src-tauri/src/commands/popover.rs` / `startup.rs` 确认 popover 窗口创建方式（WebviewWindow 独立 label）
- 确认 Tauri 2.x `app.emit` 广播所有 webview（官方行为，`emit_to` 才限定）
- 推测可达。若代码确认 popover 非 webview 或 emit 不广播 → S3 降级为 `visibilitychange` + 5s `usePolling` 兜底

## 验收

1. Logs 列表：空闲页可见时 0 IPC（无请求不刷）；高峰 500ms 内出现新行
2. Logs 详情：打开流式日志，1s 内更新终态；5s 兜底
3. popover：开窗期间有请求，1s 内卡片数据更新（跨 webview 验证通过）；或降级方案 5s 内更新
4. 30s 配置预览轮询不动
5. `cargo test` + `cargo clippy --all-targets -- -D warnings` + `yarn build` + `check-i18n.mjs` 全绿
6. 无新 warning（block future-incompat 除外）

## 不改

- 后端 `log.rs` emit 逻辑（频率 / 节流）
- CRUD commands（platform/group 无 emit）
- `onProxyLogUpdated` 封装（已 500ms debounce，不改默认值）
- PopoverConfigTab / TrayConfigTab 30s 轮询
- DOM 事件总线（`aidog-platform-test-completed` 等）

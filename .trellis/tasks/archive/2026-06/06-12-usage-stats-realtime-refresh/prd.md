# 使用统计实时刷新

## Goal

请求完成后，Platforms / Groups / Stats 页的使用统计自动实时更新，无需手动刷新/切页。

## 现状

- 前端：Platforms `load()`(Platforms.tsx:1067, `useEffect(...,[])` 仅 mount)、Stats `load`(Stats.tsx:106 useCallback)、Groups（usageStats Groups.tsx:142）——**只加载一次，无 setInterval、无事件监听**（grep 无 `listen(`）。
- 后端：`upsert_log(state, log, settings)`(proxy.rs) 每条请求写 proxy_log；ProxyState 持 AppHandle，已有 `app.emit("tray-refresh")`(proxy.rs:159, 仅 estimate/quota 路径)。

## Decision

**事件驱动**（非轮询）：后端每条日志写库后 emit 事件 → 前端三页监听，debounce 重载统计。比轮询更实时、开销低。

## Requirements

- R1 后端：`upsert_log` 成功 upsert proxy_log 后，经 AppHandle emit 事件 `"proxy-log-updated"`（payload 可空或含 platform_id）。logging 禁用时不写库也不 emit（无统计可更新，可接受）。
- R2 前端：Platforms / Stats / Groups 各自 `listen("proxy-log-updated", ...)`（`@tauri-apps/api/event`），收到事件 debounce(~500ms) 后调各自 load/刷新统计；`useEffect` 卸载时 unlisten。
- R3 debounce 合并突发（多请求短时间内只刷一次）。
- R4 不破坏现有手动刷新/加载；不引入持续轮询。

## Acceptance Criteria

- [ ] 发一条请求 → Platforms 卡片统计(请求/成本/成功率/余额) 自动更新，无需切页。
- [ ] Stats 页在事件后自动重查（尊重当前时间/筛选条件）。
- [ ] Groups 聚合统计自动更新。
- [ ] 突发多请求只触发合并后的少量刷新（debounce 生效）。
- [ ] 监听器在页面卸载时正确 unlisten，无泄漏/重复监听。
- [ ] typecheck 0；cargo check 通过。

## Definition of Done

- 跨层事件契约一致（事件名 `proxy-log-updated` 后端 emit / 前端 listen 一致）。
- Tauri 其他命令契约不变。
- 无持续轮询；debounce + cleanup 正确。

## Out of Scope

- 不改统计 SQL / est_cost 逻辑。
- 不改 tray-refresh 既有逻辑（可另 emit 或复用，优先新事件名避免混淆）。
- 不做 WebSocket / 长轮询。

## Technical Notes

- 后端 emit 点：`upsert_log` 内 `upsert_proxy_log` 成功后；用 ProxyState 的 app handle（参考 proxy.rs:159 既有 emit 写法）。
- 前端：`import { listen } from "@tauri-apps/api/event"`；debounce 用 ref + setTimeout 或现有 util。
- 复用各页既有 load 函数（Platforms.tsx:1067 / Stats.tsx:106 / Groups load）。


## s5-stats-throttle：前提证伪，无改动交付（2026-07-29）

subtask 描述称 `Stats.tsx:209` 的 `onProxyLogUpdated` 逐条事件触发全量 `loadFilterOptions()`、无节流。
**实测不成立** —— `src/services/api/proxy.ts:113` 的 `onProxyLogUpdated(callback, debounceMs = 500)`
本身内建 debounce（`:116-119` 每次事件 clearTimeout + 重设 500ms timer），是 trailing-edge 语义：
事件流停止 500ms 后才执行一次。`Stats.tsx:198`（load）与 `:209`（loadFilterOptions）走的是**同一个**
封装，非两套写法。cleanup 由 `onProxyLogUpdated` 返回值提供（`proxy.ts:121-124` 清 timer + 从
`proxyLogSubscribers` Set 注销），`useEffect` 直接 return 它，卸载自动清理。

三条验收在现状下已全部满足，故 **done 但零改动**。

### 遗留观察项（未修，收益不足以单开 subtask）

debounce 后仍是「持续转发期间每 500ms 拉一次 `groupDetailApi.list()` + `platformApi.list()` 全量」。
平台/分组列表在转发过程中几乎不变，用日志事件驱动重拉属**事件源错配**；正解是改由平台/分组
变更事件驱动。但两个 list 的 payload 量级（几十条配置项）远小于 s4 治的 200 行完整日志，
且改动会波及跨页事件契约，YAGNI 不做。

### 顺带清点：`onProxyLogUpdated` 订阅点

10+ 处（`popover.tsx:155` / `usePlatformsState.ts:707` / `Home.tsx:134` / `usePopoverConfig.ts:120` /
`useGroupData.ts:246` / `TrayConfigTab.tsx:208` / `useLogsList.ts:38` / `useLogsDetail.ts:99` /
`RequestLog.tsx:140,168`），全部走 `proxy.ts:97` `ensureProxyLogListener` 的**单例 listen** 扇出，
符合 memory `singleton-event-hub` 的既有设计，无 N-listener 问题。

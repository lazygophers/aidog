// proxy.ts — 从 services/api.ts 拆出（arch-redesign）；纯移动，零逻辑变更。

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ProxySettings, ProxyClientSettings, ProxyLogSummary, ProxyLogDetail, ProxyLogSettings, ProxyTimeoutSettings, ProxyLogFilter, RequestLogSummary } from "./types";


// ─── Proxy API ─────────────────────────────────────────────

export const proxyApi = {
  start: (port: number) => invoke<string>("proxy_start", { port }),
  stop: () => invoke<void>("proxy_stop"),
  status: () => invoke<boolean>("proxy_status"),
  getSettings: () => invoke<ProxySettings>("proxy_get_settings"),
  setAutostart: (enabled: boolean) =>
    invoke<void>("proxy_set_autostart", { enabled }),
  setBindLan: (enabled: boolean) =>
    invoke<void>("proxy_set_bind_lan", { enabled }),
  setAutolaunch: (enabled: boolean) =>
    invoke<void>("app_set_autolaunch", { enabled }),
  getAutolaunch: () => invoke<boolean>("app_get_autolaunch"),
  setSilentLaunch: (enabled: boolean) =>
    invoke<void>("app_set_silent_launch", { enabled }),
  getProxyClientSettings: () => invoke<ProxyClientSettings>("proxy_client_get_settings"),
  setProxyClientSettings: (settings: ProxyClientSettings) =>
    invoke<void>("proxy_client_set_settings", { settings }),
};

// ─── Claude Code Config Export ─────────────────────────────



// ─── Proxy Log API ─────────────────────────────────────────

export const proxyLogApi = {
  list: (limit = 50, offset = 0) =>
    invoke<ProxyLogSummary[]>("proxy_log_list", { limit, offset }),
  listFiltered: (filter: ProxyLogFilter, limit = 50, offset = 0) =>
    invoke<ProxyLogSummary[]>("proxy_log_list_filtered", { filter, limit, offset }),
  get: (id: string) =>
    invoke<ProxyLogDetail | null>("proxy_log_get", { id }),
  clear: () => invoke<void>("proxy_log_clear"),
  cleanupExpired: () => invoke<void>("proxy_log_cleanup_expired"),
  count: () => invoke<number>("proxy_log_count"),
  countFiltered: (filter: ProxyLogFilter) =>
    invoke<number>("proxy_log_count_filtered", { filter }),
  getSettings: () =>
    invoke<ProxyLogSettings>("proxy_log_settings_get"),
  setSettings: (settings: ProxyLogSettings) =>
    invoke<void>("proxy_log_settings_set", { settings }),
};

// ─── Request Log API (cli-proxy test/quota page) ───────────
// request_log_list 后端默认 sources=[test,quota]（db 层兜底；filter.sources=None 时）。
// filter 显式传 sources（含空 Vec）则尊重前端值。返回 RequestLogSummary（含 provider 归属）。

export const requestLogApi = {
  list: (filter?: ProxyLogFilter, limit = 50, offset = 0) =>
    invoke<RequestLogSummary[]>("request_log_list", { filter: filter ?? {}, limit, offset }),
};

// ─── Proxy Timeout API ──────────────────────────────────────



// ─── Proxy Timeout API ──────────────────────────────────────

export const proxyTimeoutApi = {
  get: () => invoke<ProxyTimeoutSettings>("proxy_timeout_get"),
  set: (settings: ProxyTimeoutSettings) =>
    invoke<void>("proxy_timeout_set", { settings }),
};

// ─── Middleware Rule Engine API (C1 契约冻结点) ─────────────
// 字段名与 Rust serde（src-tauri/src/gateway/models.rs MiddlewareRule/MiddlewareSettings
// + 枚举 RuleType/RuleScope/MatchType/RuleAction）严格 snake_case 一致。
// 契约由 C1 冻结，C5(UI) 仅消费不改。设计见 design.md。
// 注：熔断器已移出中间件层（归 group 独立 task），MiddlewareSettings 不含 breaker。

/** 规则类型（8 类中间件能力）。 */


export const PROXY_LOG_UPDATED = "proxy-log-updated";

// ─── proxy-log-updated hub (single shared listen per JS context) ────────
// ponytail: 1 underlying Tauri listen() per webview, fan-out to N subscribers.
// Previously each page called listen() separately (7 concurrent listeners in
// main window). Each webview has its own JS context → its own module state:
// main window = 1 listener for all pages; popover window = 1 for its own.
// Subscribers carry their own debounce; hub just fans out the raw event with
// platform_id (Rust emit payload = platform_id number).
type ProxyLogSubscriber = (platformId: number | null) => void;
const proxyLogSubscribers = new Set<ProxyLogSubscriber>();
let proxyLogListenPromise: Promise<UnlistenFn> | null = null;

function ensureProxyLogListener(): Promise<UnlistenFn> {
  if (!proxyLogListenPromise) {
    proxyLogListenPromise = listen(PROXY_LOG_UPDATED, (event) => {
      const pid = typeof event.payload === "number" ? event.payload : null;
      proxyLogSubscribers.forEach((cb) => {
        try { cb(pid); } catch (e) { console.error(e); }
      });
    });
  }
  return proxyLogListenPromise;
}

/**
 * 监听 proxy-log-updated（共享单 listener），debounce 合并突发后调 callback。
 * 返回 cleanup 函数：清 timer + 注销 subscriber，供 useEffect cleanup 使用。
 */
export function onProxyLogUpdated(callback: () => void, debounceMs = 500): () => void {
  let timer: ReturnType<typeof setTimeout> | null = null;
  const wrapped: ProxyLogSubscriber = () => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => { callback(); }, debounceMs);
  };
  proxyLogSubscribers.add(wrapped);
  ensureProxyLogListener().catch((e) => console.error(e));
  return () => {
    proxyLogSubscribers.delete(wrapped);
    if (timer) clearTimeout(timer);
  };
}

// ─── Skills API ────────────────────────────────────────────
// 字段名严格 snake_case，与 Rust gateway/skills.rs 模型一一对齐（cross-layer-rules）。

/** 目标 agent（决定 --agent 参数 + 本地配置目录）。 */


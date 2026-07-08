// proxy.ts — 从 services/api.ts 拆出（arch-redesign）；纯移动，零逻辑变更。

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ProxySettings, ProxyClientSettings, ProxyLogSummary, ProxyLogDetail, ProxyLogSettings, ProxyTimeoutSettings, ProxyLogFilter } from "./types";


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

/**
 * 监听 proxy-log-updated，debounce 合并突发后调 callback。
 * 返回 cleanup 函数：清 timer + unlisten，供 useEffect cleanup 使用。
 */


export function onProxyLogUpdated(callback: () => void, debounceMs = 500): () => void {
  let timer: ReturnType<typeof setTimeout> | null = null;
  const unlistenPromise = listen(PROXY_LOG_UPDATED, () => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => { callback(); }, debounceMs);
  });
  return () => {
    if (timer) clearTimeout(timer);
    unlistenPromise.then((un) => un()).catch((e) => console.error(e));
  };
}

// ─── Skills API ────────────────────────────────────────────
// 字段名严格 snake_case，与 Rust gateway/skills.rs 模型一一对齐（cross-layer-rules）。

/** 目标 agent（决定 --agent 参数 + 本地配置目录）。 */


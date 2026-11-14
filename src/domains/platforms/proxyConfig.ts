// proxyConfig — 全局代理配置（ProxyClientSettings）的轻量共享读：host 是否已配置（非空）。
// 消费者：平台行/卡片「代理」徽标、平台编辑页「使用代理」提示、新建平台预填（issue #64 T02/T03）。
// docPromise 单次 RPC 缓存 idiom：首次读取共享一次 invoke；useSystemSettings 加载/保存后
// 调 broadcastProxyConfigChanged() 广播事件，挂载中的消费者重取，避免跨页缓存漂移。
import { useEffect, useState } from "react";
import { proxyApi } from "../../services/api";

/** 代理配置变更广播事件（设置页加载/保存后触发，消费者重取 host）。 */
export const PROXY_CONFIG_EVENT = "aidog-proxy-config-changed";

let hostCache: string | null = null;
let inflight: Promise<string> | null = null;

function fetchHost(): Promise<string> {
  const p = proxyApi.getProxyClientSettings().then(pc => {
    hostCache = pc.host ?? "";
    return hostCache;
  }).catch(() => hostCache ?? "");
  inflight = p.finally(() => { inflight = null; });
  return p;
}

function currentHost(): Promise<string> {
  if (hostCache != null && !inflight) return Promise.resolve(hostCache);
  return inflight ?? fetchHost();
}

/** host 是否非空（= 代理已配置）。异步一次性读取，供新建平台表单预填（T03）。 */
export function getProxyHostConfigured(): Promise<boolean> {
  return currentHost().then(h => h.trim() !== "");
}

/** 设置页在加载/保存代理配置后调用：清缓存 + 广播，已挂载的徽标/提示随之刷新。 */
export function broadcastProxyConfigChanged() {
  hostCache = null;
  window.dispatchEvent(new Event(PROXY_CONFIG_EVENT));
}

/** 全局代理地址是否已配置（host 非空）。监听广播事件，配置变更时自动重取。 */
export function useProxyConfigured(): boolean {
  const [configured, setConfigured] = useState(false);
  useEffect(() => {
    let alive = true;
    const sync = () => { currentHost().then(h => { if (alive) setConfigured(h.trim() !== ""); }); };
    sync();
    window.addEventListener(PROXY_CONFIG_EVENT, sync);
    return () => { alive = false; window.removeEventListener(PROXY_CONFIG_EVENT, sync); };
  }, []);
  return configured;
}

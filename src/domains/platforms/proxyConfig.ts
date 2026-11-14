// proxyConfig — 全局代理配置（ProxyClientSettings）的轻量共享读：host 是否已配置（非空）。
// 消费者：平台行/卡片「代理」徽标、平台编辑页「使用代理」提示、新建平台预填（issue #64 T02/T03）。
// docPromise 单次 RPC 缓存 idiom：首次读取共享一次 invoke；useSystemSettings 加载/保存后
// 调 broadcastProxyConfigChanged() 广播事件，挂载中的消费者重取，避免跨页缓存漂移。
import { useEffect, useState } from "react";
import { proxyApi, platformApi, serializeProxyEnabled } from "../../services/api";

/** 代理配置变更广播事件（设置页加载/保存后触发，消费者重取 host）。 */
export const PROXY_CONFIG_EVENT = "aidog-proxy-config-changed";

let hostCache: string | null = null;
let gen = 0;
let inflight: { gen: number; value: Promise<string> } | null = null;

function fetchHost(): Promise<string> {
  const g = gen;
  const value = proxyApi
    .getProxyClientSettings()
    .then(pc => {
      // 广播后 gen 已推进：在途旧请求的结果作废，不回写缓存（review smell 5 轻微竞态修复）。
      if (g === gen) hostCache = pc.host ?? "";
      return pc.host ?? "";
    })
    .catch(() => hostCache ?? "");
  const entry = { gen: g, value };
  value.finally(() => {
    if (inflight === entry) inflight = null;
  });
  inflight = entry;
  return value;
}

function currentHost(): Promise<string> {
  if (hostCache != null) return Promise.resolve(hostCache);
  if (inflight && inflight.gen === gen) return inflight.value;
  return fetchHost();
}

/** host 是否非空（= 代理已配置）。异步一次性读取，供新建平台表单预填（T03）。 */
export function getProxyHostConfigured(): Promise<boolean> {
  return currentHost().then(h => h.trim() !== "");
}

/** 设置页在加载/保存代理配置后调用：作废在途请求 + 清缓存 + 广播，已挂载的徽标/提示随之刷新。 */
export function broadcastProxyConfigChanged() {
  gen += 1;
  hostCache = null;
  window.dispatchEvent(new Event(PROXY_CONFIG_EVENT));
}

/** 全局代理地址是否已配置（host 非空）。监听广播事件，配置变更后自动重取。 */
export function useProxyConfigured(): boolean {
  const [configured, setConfigured] = useState(false);
  useEffect(() => {
    let alive = true;
    const sync = () => {
      currentHost().then(h => {
        if (alive) setConfigured(h.trim() !== "");
      });
    };
    sync();
    window.addEventListener(PROXY_CONFIG_EVENT, sync);
    return () => {
      alive = false;
      window.removeEventListener(PROXY_CONFIG_EVENT, sync);
    };
  }, []);
  return configured;
}

/** 「为全部平台开启」执行体（issue #64 T02；review smell 4：移出系统设置 hook + 失败数统计）。
 * 逐平台写 extra.proxy_enabled=true（保留其余键），返回成功/失败数；写完广播平台列表刷新
 * （usePlatformsState 监听 aidog-platforms-changed）。单平台失败不中断其余。 */
export async function enableProxyForAllPlatforms(): Promise<{ ok: number; failed: number }> {
  const list = (await platformApi.list()) || [];
  let ok = 0;
  let failed = 0;
  for (const p of list) {
    try {
      await platformApi.update({ id: p.id, extra: serializeProxyEnabled(p.extra ?? "", true) });
      ok++;
    } catch {
      failed++;
    }
  }
  window.dispatchEvent(new Event("aidog-platforms-changed"));
  return { ok, failed };
}

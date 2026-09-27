// pricing.ts — 从 services/api.ts 拆出（arch-redesign）；纯移动，零逻辑变更。

import { invoke, listen, type UnlistenFn } from "../transport";
import type { ModelEntry, ModelInfoSnapshot, PriceSyncSettings } from "./types";

export const priceSyncApi = {
  get: () =>
    invoke<PriceSyncSettings>("price_sync_settings_get"),
  set: (settings: PriceSyncSettings) =>
    invoke<void>("price_sync_settings_set", { settings }),
};

// ─── 模型信息中枢（model-info 票 T2）─────────────────────────
// 数据源 model_entry / platform_preset 两表，后端 DB 空时自动回落编译期内置 registry。
// snapshot 一次拿全「聚合行 + 平台预设（含品牌字段）」，模型信息页首屏不做二次 RPC 拼装。

export const modelInfoApi = {
  /** 平台维度：传 platformCode 只取该平台条目；不传 = 全量。 */
  list: (platformCode?: string) =>
    invoke<ModelEntry[]>("model_entry_list", { platformCode: platformCode ?? null }),
  get: (platformCode: string, modelId: string) =>
    invoke<ModelEntry | null>("model_entry_get", { platformCode, modelId }),
  /** 模型维度 + 平台预设一次性快照。 */
  snapshot: () => invoke<ModelInfoSnapshot>("model_info_snapshot"),
};

// ─── Realtime Events ───────────────────────────────────────
// 后端每条 proxy_log 写库成功后 emit "proxy-log-updated"（payload 为 platform_id）。
// Platforms / Stats / Groups 三页用此事件实时刷新统计（hub 在 proxy.ts::onProxyLogUpdated）。

// ─── registry-updated hub ─────────────────────────────────
// 后端注册表远程同步落库且有实际变更（added+updated>0）时 emit "registry-updated"（无 payload）。
// 与 proxy-log-updated hub 同 idiom：单底层 listen 扇出，订阅者自带 debounce。
export const REGISTRY_UPDATED = "registry-updated";

type RegistrySubscriber = () => void;
const registrySubscribers = new Set<RegistrySubscriber>();
let registryListenPromise: Promise<UnlistenFn> | null = null;

function ensureRegistryListener(): Promise<UnlistenFn> {
  if (!registryListenPromise) {
    registryListenPromise = listen(REGISTRY_UPDATED, () => {
      registrySubscribers.forEach((cb) => {
        try { cb(); } catch (e) { console.error(e); }
      });
    });
  }
  return registryListenPromise;
}

/**
 * 监听 registry-updated（共享单 listener），debounce 合并后调 callback。
 * 返回 cleanup 函数，供 useEffect cleanup 使用。
 */
export function onRegistryUpdated(callback: () => void, debounceMs = 500): () => void {
  let timer: ReturnType<typeof setTimeout> | null = null;
  const wrapped: RegistrySubscriber = () => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => { callback(); }, debounceMs);
  };
  registrySubscribers.add(wrapped);
  ensureRegistryListener().catch((e) => console.error(e));
  return () => {
    registrySubscribers.delete(wrapped);
    if (timer) clearTimeout(timer);
  };
}


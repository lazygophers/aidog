// pricing.ts — 从 services/api.ts 拆出（arch-redesign）；纯移动，零逻辑变更。

import { invoke } from "@tauri-apps/api/core";
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
// Platforms / Stats / Groups 三页用此事件实时刷新统计。

/** 后端代理日志更新事件名（后端 emit / 前端 listen 必须一致） */


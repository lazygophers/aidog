// pricing.ts — 从 services/api.ts 拆出（arch-redesign）；纯移动，零逻辑变更。

import { invoke } from "@tauri-apps/api/core";
import type { PriceSyncSettings } from "./types";

export const priceSyncApi = {
  get: () =>
    invoke<PriceSyncSettings>("price_sync_settings_get"),
  set: (settings: PriceSyncSettings) =>
    invoke<void>("price_sync_settings_set", { settings }),
};

// ─── Realtime Events ───────────────────────────────────────
// 后端每条 proxy_log 写库成功后 emit "proxy-log-updated"（payload 为 platform_id）。
// Platforms / Stats / Groups 三页用此事件实时刷新统计。

/** 后端代理日志更新事件名（后端 emit / 前端 listen 必须一致） */


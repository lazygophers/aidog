// stats.ts — 从 services/api.ts 拆出（arch-redesign）；纯移动，零逻辑变更。

import { invoke } from "@tauri-apps/api/core";
import type { StatsQuery, StatsResult, StatsSettings } from "./types";

export const statsApi = {
  query: (query: StatsQuery) =>
    invoke<StatsResult>("stats_query", { query }),
  /** 批量查询：一次 IPC 拉多卡数据，结果顺序与 queries 一一对应。浮窗 N 卡用，消除 fan-out。 */
  queryBatch: (queries: StatsQuery[]) =>
    invoke<StatsResult[]>("stats_query_batch", { queries }),
  /** 清空聚合表并从 proxy_log 全量重建（启用日志后修复历史聚合）。 */
  rebuildFromLogs: () => invoke<void>("stats_rebuild_from_logs"),
};

// ─── Stats Settings (聚合表 retention) ────────────────────


export const statsSettingsApi = {
  get: () => invoke<StatsSettings>("stats_settings_get"),
  set: (settings: StatsSettings) =>
    invoke<void>("stats_settings_set", { settings }),
};

// ─── Model Testing Types & API ───────────────────────────


// stats.ts — 从 services/api.ts 拆出（arch-redesign）；纯移动，零逻辑变更。

import { invoke } from "../transport";
import type { QuotaSnapshot, QuotaSnapshotsQuery, ScatterHistogram, ScatterHistogramQuery, StatsQuery, StatsResult, StatsSettings } from "./types";

export const statsApi = {
  query: (query: StatsQuery) =>
    invoke<StatsResult>("stats_query", { query }),
  /** 批量查询：一次 IPC 拉多卡数据，结果顺序与 queries 一一对应。浮窗 N 卡用，消除 fan-out。 */
  queryBatch: (queries: StatsQuery[]) =>
    invoke<StatsResult[]>("stats_query_batch", { queries }),
  /** 清空聚合表并从 proxy_log 全量重建（启用日志后修复历史聚合）。 */
  rebuildFromLogs: () => invoke<void>("stats_rebuild_from_logs"),
  /** 散点直方图（#33）：服务端 bin 化的 (duration × cost) 计数矩阵，前端 ScatterChart 直接渲染。 */
  scatterHistogram: (query: ScatterHistogramQuery) =>
    invoke<ScatterHistogram>("scatter_histogram", { query }),
  /** 配额快照序列（#34）：时间窗内某平台（或全部平台）的余额历史，供配额趋势图。 */
  quotaSnapshots: (query: QuotaSnapshotsQuery) =>
    invoke<QuotaSnapshot[]>("quota_snapshots", { query }),
};

// ─── Stats Settings (聚合表 retention) ────────────────────


export const statsSettingsApi = {
  get: () => invoke<StatsSettings>("stats_settings_get"),
  set: (settings: StatsSettings) =>
    invoke<void>("stats_settings_set", { settings }),
};

// ─── Model Testing Types & API ───────────────────────────


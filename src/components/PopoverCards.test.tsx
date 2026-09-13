// ── Popover 三件套（#38 T8）：查询收集（queryBatch 批量通道）+ 曲线/环形/热力条渲染 ──
// 测试 i18n 回传 key 本身（见 test/render），断言用 key。
import { describe, it, expect } from "vitest";
import { render, screen } from "../test/render";
import {
  collectStatsQueries,
  renderItem,
  type PopoverData,
  type PopoverStatsCtx,
} from "./PopoverCards";
import type { PopoverConfig, PopoverItem, StatsBucket, StatsResult } from "../services/api";

// ─── Fixtures ────────────────────────────────────────────────

const HOUR = 3_600_000;

function bucket(h: number, cost: number, req = h): StatsBucket {
  // 今日 hourly 桶串（本地日期无关，只借 HH 槽位）
  return {
    time_bucket: `2026-09-13 ${String(h).padStart(2, "0")}:00:00`,
    total_requests: req,
    success_count: req,
    error_count: 0,
    input_tokens: 100,
    output_tokens: 100,
    cache_tokens: 0,
    avg_duration_ms: 10,
    total_cost: cost,
  };
}

function makeItem(type: PopoverItem["item_type"], extra: Partial<PopoverItem> = {}): PopoverItem {
  return { id: `i-${type}`, item_type: type, visible: true, order: 0, row: 0, size: "m", ...extra };
}

const EMPTY_DATA: PopoverData = {
  config: { items: [], rows: [] },
  entries: [],
  today_stats: { cost: 0, tokens: 0, cache_rate: 0, total_requests: 0 } as PopoverData["today_stats"],
  platform_today: [],
  proxy_running: true,
  proxy_port: 0,
};

const EMPTY_GROUPS: Parameters<typeof renderItem>[3] = null;
const CTX = (map: Map<string, StatsResult>, loaded = true): PopoverStatsCtx => ({ map, loaded });
const t = (k: string) => k; // 回传 key 本身（默认值/插值忽略），断言走 key

// ─── collectStatsQueries：新卡进批量通道、参数正确 ────────────

describe("collectStatsQueries: popover trio", () => {
  it("platform_share queries with series_by=platform and the item window", () => {
    const { itemIds, queries } = collectStatsQueries({
      items: [makeItem("platform_share", { time_window: "7d" })],
    } as PopoverConfig);
    expect(itemIds).toEqual(["i-platform_share"]);
    expect(queries.length).toBe(1);
    expect(queries[0].series_by).toBe("platform");
    expect(queries[0].granularity).toBe("daily"); // 7d → daily
  });

  it("hour_heatbar forces today + hourly regardless of configured window", () => {
    const { queries } = collectStatsQueries({
      items: [makeItem("hour_heatbar", { time_window: "30d" })],
    } as PopoverConfig);
    expect(queries.length).toBe(1);
    expect(queries[0].granularity).toBe("hourly");
    const spanDays = (queries[0].end - queries[0].start) / 86_400_000;
    expect(spanDays).toBeLessThanOrEqual(1); // 今日窗
  });

  it("non-stats item types produce no queries", () => {
    const { queries } = collectStatsQueries({
      items: [makeItem("proxy_status")],
    } as PopoverConfig);
    expect(queries.length).toBe(0);
  });
});

// ─── renderItem：三件套渲染 ──────────────────────────────────

describe("renderItem: popover trio", () => {
  it("cost_trend renders mini LineChart (public layer, no old inline SVG)", () => {
    const stats: StatsResult = mkResult([bucket(1, 1), bucket(2, 2), bucket(3, 3)]);
    const { container } = render(
      <>{renderItem(makeItem("cost_trend"), EMPTY_DATA, [], EMPTY_GROUPS, t, CTX(new Map([["i-cost_trend", stats]])))}</>,
    );
    expect(container.querySelectorAll(".recharts-line-curve").length).toBe(1);
    expect(container.querySelector("svg")).toBeTruthy();
  });

  it("platform_share aggregates series into donut with legend rows", () => {
    const stats = mkResult([], [
      { name: "Alpha", buckets: [bucket(1, 3)] },
      { name: "Beta", buckets: [bucket(1, 1)] },
    ]);
    // 扇区本体受 rAF 动画门控（jsdom 不挂载），断言走无动画路径的图例行 + 环形容器
    const { container } = render(
      <>{renderItem(makeItem("platform_share"), EMPTY_DATA, [], EMPTY_GROUPS, t, CTX(new Map([["i-platform_share", stats]])))}</>,
    );
    expect(container.querySelector(".recharts-surface")).toBeTruthy();
    expect(screen.getByText("Alpha")).toBeTruthy();
    expect(screen.getByText("Beta")).toBeTruthy();
  });

  it("platform_share with <2 effective slices shows honest empty state", () => {
    const stats = mkResult([], [{ name: "Alpha", buckets: [bucket(1, 3)] }]);
    render(
      <>{renderItem(makeItem("platform_share"), EMPTY_DATA, [], EMPTY_GROUPS, t, CTX(new Map([["i-platform_share", stats]])))}</>,
    );
    expect(screen.getByText("charts.noData")).toBeTruthy();
  });

  it("hour_heatbar renders 24-cell heat strip", () => {
    const stats = mkResult([bucket(9, 1, 12), bucket(20, 2, 40)]);
    const { container } = render(
      <>{renderItem(makeItem("hour_heatbar"), EMPTY_DATA, [], EMPTY_GROUPS, t, CTX(new Map([["i-hour_heatbar", stats]])))}</>,
    );
    expect(container.querySelectorAll("[data-heat-hour]").length).toBe(24);
  });

  it("hour_heatbar empty buckets show honest empty state, not a zero-value fake bar", () => {
    const stats = mkResult([]);
    render(
      <>{renderItem(makeItem("hour_heatbar"), EMPTY_DATA, [], EMPTY_GROUPS, t, CTX(new Map([["i-hour_heatbar", stats]])))}</>,
    );
    expect(screen.getByText("popover.noUsageToday")).toBeTruthy();
    expect(document.querySelectorAll("[data-heat-hour]").length).toBe(0);
  });

  it("stats not yet loaded shows loading for the trio", () => {
    for (const ty of ["cost_trend", "platform_share", "hour_heatbar"] as const) {
      const { unmount } = render(
        <>{renderItem(makeItem(ty), EMPTY_DATA, [], EMPTY_GROUPS, t, CTX(new Map(), false))}</>,
      );
      expect(screen.getByText("common.loading")).toBeTruthy();
      unmount();
    }
  });
});

function mkResult(buckets: StatsBucket[], series: { name: string; buckets: StatsBucket[] }[] = []): StatsResult {
  return {
    overview: {
      total_requests: 0,
      success_count: 0,
      error_count: 0,
      input_tokens: 0,
      output_tokens: 0,
      cache_tokens: 0,
      total_cost: 0,
      success_rate: 0,
      avg_duration_ms: 0,
    },
    buckets,
    dimension_data: [],
    available_models: [],
    series,
  };
}

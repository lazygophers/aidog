// T3 图表数据映射测试（先红）：订阅曲线窗口/拟合/校准偏差、按量日柱、失败分类、模型分布。
import { describe, it, expect } from "vitest";
import {
  monotonePath,
  cycleMsForName,
  buildSubscriptionCurve,
  buildDailyBars,
  classifyFailure,
  classifyFailures,
  topModels,
  type SeriesPointLike,
} from "./chartMath";

const HOUR = 3_600_000;
const DAY = 24 * HOUR;
// 2026-11-20 12:00 UTC
const NOW = Date.UTC(2026, 10, 20, 12, 0, 0);

function seriesPoint(time: string, tokens: number, cost = 0, requests = 1): SeriesPointLike {
  return {
    time, requests, success: requests, input_tokens: tokens,
    output_tokens: 0, cache_tokens: 0, est_cost: cost, duration_ms: 100,
  };
}

describe("cycleMsForName", () => {
  it("five_hour → 5 小时；weekly → 7 天；month → 30 天；day → 24h；未知 → 7 天缺省", () => {
    expect(cycleMsForName("five_hour")).toBe(5 * HOUR);
    expect(cycleMsForName("weekly_limit")).toBe(7 * DAY);
    expect(cycleMsForName("mcp_monthly")).toBe(30 * DAY);
    expect(cycleMsForName("daily")).toBe(DAY);
    expect(cycleMsForName("whatever")).toBe(7 * DAY);
  });
});

describe("monotonePath", () => {
  it("空/单点 → M 指令；多点 → 端点在场且 C 段数 = 点数-1", () => {
    expect(monotonePath([])).toBe("");
    expect(monotonePath([{ x: 10, y: 20 }])).toBe("M 10 20");
    const p = monotonePath([
      { x: 0, y: 0 }, { x: 10, y: 5 }, { x: 20, y: 12 },
    ]);
    expect(p.startsWith("M 0 0")).toBe(true);
    expect(p.includes("20 12")).toBe(true);
    expect((p.match(/C /g) ?? []).length).toBe(2);
  });

  it("单调数据不产生过冲（控制点 y 值不越过相邻数据 y）", () => {
    const pts = [
      { x: 0, y: 0 }, { x: 100, y: 30 }, { x: 200, y: 40 }, { x: 300, y: 90 },
    ];
    const p = monotonePath(pts);
    const nums = p.match(/-?\d+(\.\d+)?/g)?.map(Number) ?? [];
    // 所有 y 控制值都应落在 [0, 90] 区间内（无过冲）
    const ys: number[] = [];
    const m = p.matchAll(/[C\s](-?[\d.]+)[\s,](-?[\d.]+)/g);
    for (const seg of m) ys.push(Number(seg[2]));
    void nums;
    for (const y of ys) {
      expect(y).toBeGreaterThanOrEqual(-0.001);
      expect(y).toBeLessThanOrEqual(90.001);
    }
  });
});

describe("buildSubscriptionCurve", () => {
  // 完整本地日窗口；series 查询覆盖其起点。
  const weeklyStart = new Date(NOW - 6 * DAY);
  weeklyStart.setHours(0, 0, 0, 0);
  const weeklyReset = new Date(weeklyStart.getTime() + 7 * DAY).toISOString();
  const seriesWindowStart = NOW - 7 * DAY + HOUR;
  const series: SeriesPointLike[] = [
    seriesPoint("2026-11-13", 100),
    seriesPoint("2026-11-14", 200),
    seriesPoint("2026-11-16", 300),
    seriesPoint("2026-11-19", 400),
  ];
  const quotaTiers = [
    { name: "five_hour", utilization: 10, limit: null, resets_at: new Date(NOW + HOUR).toISOString() },
    { name: "weekly_limit", utilization: 30, limit: 2000, resets_at: weeklyReset },
  ];

  it("窗口锚点：取周期最长档，windowStart = resetAt − cycle", () => {
    const c = buildSubscriptionCurve({ series, seriesWindowStart, seriesQueriedAt: NOW, quotaTiers, quotaQueriedAt: NOW, estTiers: [], nowMs: NOW });
    expect(c).not.toBeNull();
    expect(c!.resetAt).toBe(Date.parse(weeklyReset));
    expect(c!.windowStart).toBe(Date.parse(weeklyReset) - 7 * DAY);
    // 计划线 100% → 0%
    expect(c!.plan[0].pct).toBe(100);
    expect(c!.plan[1].pct).toBe(0);
  });

  it("实际剩余：按 limit 折算累计消耗，桶点递减", () => {
    const c = buildSubscriptionCurve({ series, seriesWindowStart, seriesQueriedAt: NOW, quotaTiers, quotaQueriedAt: NOW, estTiers: [], nowMs: NOW })!;
    // 11-13 的日桶早于窗口实际起点，不能把重置前消耗算进来。
    expect(c.actual.length).toBe(3);
    // 桶耗 200/300/400 token，limit 2000 → 本地记账剩余 90/75/55。
    expect(c.actual[0].pct).toBeCloseTo(90, 5);
    expect(c.actual[2].pct).toBeCloseTo(55, 5);
    // 当前值取官方快照，而不是用较早的最后一个日桶冒充实时值。
    expect(c.snapshotPct).toBeCloseTo(70, 5);
    const firstDayEnd = new Date("2026-11-14T00:00:00");
    firstDayEnd.setDate(firstDayEnd.getDate() + 1);
    firstDayEnd.setHours(0, 0, 0, 0);
    expect(c.actual[0].t).toBe(firstDayEnd.getTime()); // Keep the day bucket's real end time.
    expect(c.actual[2].t).toBeLessThan(NOW); // Yesterday's day bucket is not moved to the current cursor.
    // 递减
    for (let i = 1; i < c.actual.length; i++) {
      expect(c.actual[i].pct).toBeLessThanOrEqual(c.actual[i - 1].pct);
    }
  });

  it("历史曲线只用 tokens 同单位、完整日窗口；其它窗口只保留官方快照", () => {
    const windowStart = new Date(NOW - 6 * DAY);
    windowStart.setHours(0, 0, 0, 0);
    const resetAt = windowStart.getTime() + 7 * DAY;
    const base = {
      series, seriesWindowStart: windowStart.getTime(), seriesQueriedAt: NOW,
      quotaQueriedAt: NOW, estTiers: [], nowMs: NOW,
    };
    const countTier = buildSubscriptionCurve({
      ...base,
      estTiers: [{ name: "weekly_limit", unit: "tokens", limit: 2000, window_start: windowStart.getTime() }],
      quotaTiers: [{ name: "weekly_limit", unit: "prompt_count", utilization: 20, limit: 2000, resets_at: new Date(resetAt).toISOString() }],
    })!;
    expect(countTier.actual).toEqual([]);
    expect(countTier.snapshotPct).toBe(80);

    const incomplete = buildSubscriptionCurve({
      ...base,
      seriesWindowStart: windowStart.getTime() + 1,
      quotaTiers: [{ name: "weekly_limit", unit: "tokens", utilization: 30, limit: 2000, resets_at: new Date(resetAt).toISOString() }],
    })!;
    expect(incomplete.actual).toEqual([]);

    const partialDay = buildSubscriptionCurve({
      ...base,
      quotaTiers: [{ name: "weekly_limit", unit: "tokens", utilization: 30, limit: 2000, resets_at: new Date(resetAt + HOUR).toISOString() }],
    })!;
    expect(partialDay.actual).toEqual([]);

    const shortCycle = buildSubscriptionCurve({
      ...base,
      quotaTiers: [{ name: "five_hour", unit: "tokens", utilization: 30, limit: 2000, resets_at: new Date(NOW + HOUR).toISOString() }],
    })!;
    expect(shortCycle.actual).toEqual([]);

    const unknownCycle = buildSubscriptionCurve({
      ...base,
      quotaTiers: [{ name: "custom_window", unit: "tokens", utilization: 30, limit: 2000, resets_at: new Date(resetAt).toISOString() }],
    })!;
    expect(unknownCycle.actual).toEqual([]);
  });

  it("校准点：quota tiers → t = queried_at，pct = 100 − utilization；偏差 = 与本地曲线插值差", () => {
    const c = buildSubscriptionCurve({ series, seriesWindowStart, seriesQueriedAt: NOW, quotaTiers, quotaQueriedAt: NOW, estTiers: [], nowMs: NOW })!;
    // 仅选择中的 weekly 窗口可与图表的周期和刻度比较。
    expect(c.calibrations).toEqual([{ t: NOW, pct: 70, name: "weekly_limit", unit: "tokens" }]);
    // NOW 在最后一个日桶之后，不能外推该桶值并伪报校准偏差。
    expect(c.maxDeviation).toBeNull();
  });

  it("缺官方快照时，历史日桶保留真实时间，当前余额保持未知", () => {
    const c = buildSubscriptionCurve({
      series: [seriesPoint("2026-11-19", 400)],
      seriesWindowStart, seriesQueriedAt: NOW,
      quotaTiers: [{ name: "weekly_limit", unit: "tokens", limit: 2000, resets_at: weeklyReset }],
      quotaQueriedAt: null,
      estTiers: [],
      nowMs: NOW + HOUR,
    })!;
    expect(c.actual).toHaveLength(1);
    expect(c.actual[0].t).toBeLessThan(NOW);
    expect(c.snapshotPct).toBeNull();
    expect(c.calibrations).toEqual([]);
  });

  it("无 limit → 不投影剩余额度；有效官方快照仍可显示", () => {
    const resetAt = NOW + 3 * HOUR;
    const c = buildSubscriptionCurve({
      series: [seriesPoint("2026-11-14", 400), seriesPoint("2026-11-18", 600)],
      quotaTiers: [{ name: "weekly_limit", utilization: 30, limit: null, resets_at: new Date(resetAt).toISOString() }],
      quotaQueriedAt: NOW,
      estTiers: [],
      nowMs: NOW,
    })!;
    expect(c.actual).toEqual([]);
    expect(c.snapshotPct).toBe(70);
    expect(c.calibrations).toEqual([{ t: NOW, pct: 70, name: "weekly_limit", unit: "tokens" }]);
    expect(c.maxDeviation).toBeNull();
  });

  it("无 limit 且无官方快照 → 剩余值未知", () => {
    const resetAt = NOW + 3 * HOUR;
    const c = buildSubscriptionCurve({
      series: [seriesPoint("2026-11-18", 600)],
      quotaTiers: [{ name: "weekly_limit", limit: null, resets_at: new Date(resetAt).toISOString() }],
      quotaQueriedAt: null,
      estTiers: [],
      nowMs: NOW,
    })!;
    expect(c.actual).toEqual([]);
    expect(c.snapshotPct).toBeNull();
  });

  it("无任何窗口锚点（无 quota/est tiers）→ null", () => {
    expect(buildSubscriptionCurve({ series, quotaTiers: [], quotaQueriedAt: null, estTiers: [], nowMs: NOW })).toBeNull();
  });

  it("est tiers（window_start）可作锚点；quota 无 resets_at 时回落 est", () => {
    const ws = NOW - 2 * DAY;
    const c = buildSubscriptionCurve({
      series: [seriesPoint("2026-11-19", 100)],
      quotaTiers: [],
      quotaQueriedAt: null,
      estTiers: [{ name: "five_hour", est_utilization: 20, window_start: ws }],
      nowMs: NOW,
    })!;
    expect(c.windowStart).toBe(ws);
    expect(c.resetAt).toBe(ws + 5 * HOUR);
  });

  it("窗口外桶被排除（time < windowStart 不计入）", () => {
    const c = buildSubscriptionCurve({
      series: [seriesPoint("2026-11-01", 500), seriesPoint("2026-11-15", 100)],
      seriesWindowStart,
      seriesQueriedAt: NOW,
      quotaTiers,
      quotaQueriedAt: null,
      estTiers: [],
      nowMs: NOW,
    })!;
    expect(c.actual.length).toBe(1);
    expect(c.actual[0].pct).toBeCloseTo(95, 5);
  });
});

describe("buildDailyBars", () => {
  it("逐点映射 day/cost/requests，保持顺序", () => {
    const bars = buildDailyBars([
      seriesPoint("2026-11-14", 0, 1.5, 12),
      seriesPoint("2026-11-15", 0, 0, 0),
    ]);
    expect(bars).toEqual([
      { day: "2026-11-14", cost: 1.5, requests: 12 },
      { day: "2026-11-15", cost: 0, requests: 0 },
    ]);
  });
});

describe("classifyFailure / classifyFailures", () => {
  it("状态码分类：429 rate；401/403/402 auth；400/404/422/451 model；5xx service", () => {
    expect(classifyFailure({ status_code: 429, model: "m", created_at: 1 })).toBe("rate");
    expect(classifyFailure({ status_code: 401, model: "m", created_at: 1 })).toBe("auth");
    expect(classifyFailure({ status_code: 403, model: "m", created_at: 1 })).toBe("auth");
    expect(classifyFailure({ status_code: 402, model: "m", created_at: 1 })).toBe("auth");
    expect(classifyFailure({ status_code: 400, model: "m", created_at: 1 })).toBe("model");
    expect(classifyFailure({ status_code: 422, model: "m", created_at: 1 })).toBe("model");
    expect(classifyFailure({ status_code: 451, model: "m", created_at: 1 })).toBe("model");
    expect(classifyFailure({ status_code: 502, model: "m", created_at: 1 })).toBe("service");
    expect(classifyFailure({ status_code: 0, model: "m", created_at: 1 })).toBe("service");
  });

  it("聚合计数 + recent 取 created_at 降序前 3", () => {
    const rows = [
      { status_code: 429, model: "a", created_at: 1 },
      { status_code: 401, model: "a", created_at: 2 },
      { status_code: 500, model: "b", created_at: 3 },
      { status_code: 400, model: "c", created_at: 4 },
      { status_code: 503, model: "d", created_at: 5 },
    ];
    const s = classifyFailures(rows);
    expect(s.counts).toEqual({ service: 2, model: 1, rate: 1, auth: 1 });
    expect(s.recent.length).toBe(3);
    expect(s.recent.map(r => r.created_at)).toEqual([5, 4, 3]);
    expect(s.recent[0].cls).toBe("service");
  });
});

describe("topModels", () => {
  it("降序截断 + pct 占比（总数含被截断部分）", () => {
    const rows = [
      { model: "m1", requests: 60 },
      { model: "m2", requests: 30 },
      { model: "m3", requests: 10 },
    ];
    const top = topModels(rows, 2);
    expect(top.length).toBe(2);
    expect(top[0]).toEqual({ model: "m1", requests: 60, pct: 60 });
    expect(top[1].pct).toBe(30);
  });

  it("总请求 0 → pct 全 0", () => {
    expect(topModels([{ model: "m", requests: 0 }], 8)).toEqual([{ model: "m", requests: 0, pct: 0 }]);
  });
});

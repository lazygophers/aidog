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
  // 窗口：2026-11-13 → 2026-11-20（weekly tier，resets_at = NOW + 3h）
  const weeklyReset = new Date(NOW + 3 * HOUR).toISOString();
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
    const c = buildSubscriptionCurve({ series, quotaTiers, quotaQueriedAt: NOW, estTiers: [], nowMs: NOW });
    expect(c).not.toBeNull();
    expect(c!.resetAt).toBe(Date.parse(weeklyReset));
    expect(c!.windowStart).toBe(Date.parse(weeklyReset) - 7 * DAY);
    // 计划线 100% → 0%
    expect(c!.plan[0].pct).toBe(100);
    expect(c!.plan[1].pct).toBe(0);
  });

  it("实际剩余：按 limit 折算累计消耗，桶点递减", () => {
    const c = buildSubscriptionCurve({ series, quotaTiers, quotaQueriedAt: NOW, estTiers: [], nowMs: NOW })!;
    // 窗口内全部 4 桶（11-13..11-19 都 ≥ windowStart 当地零点）
    expect(c.actual.length).toBe(4);
    // 桶耗 100/200/300/400 token，limit 2000 → 剩余 95/85/70/50
    expect(c.actual[0].pct).toBeCloseTo(95, 5);
    expect(c.actual[3].pct).toBeCloseTo(50, 5);
    expect(c.nowPct).toBeCloseTo(50, 5);
    // 递减
    for (let i = 1; i < c.actual.length; i++) {
      expect(c.actual[i].pct).toBeLessThanOrEqual(c.actual[i - 1].pct);
    }
  });

  it("校准点：quota tiers → t = queried_at，pct = 100 − utilization；偏差 = 与本地曲线插值差", () => {
    const c = buildSubscriptionCurve({ series, quotaTiers, quotaQueriedAt: NOW, estTiers: [], nowMs: NOW })!;
    // 两档各一个校准点
    expect(c.calibrations.length).toBe(2);
    expect(c.calibrations[0].t).toBe(NOW);
    expect(c.calibrations[0].pct).toBeCloseTo(90, 5); // 100-10
    expect(c.calibrations[1].pct).toBeCloseTo(70, 5); // 100-30
    // NOW 在最后桶（11-19 本地零点）之后 → 本地插值 = 50 → 偏差 max(|50-90|, |50-70|) = 40
    expect(c.maxDeviation).toBeCloseTo(40, 5);
  });

  it("无 limit → 按 elapsedFrac 投影总量；剩余随计划线", () => {
    // 已流逝 6.5/7 天；累计 1000 token → 投影总量 ≈ 1000/(6.5/7)
    const start = NOW - (6.5 * DAY) / (7 + 3 / 24) * (7 + 3 / 24);
    const resetAt = NOW + 3 * HOUR;
    const windowStart = resetAt - 7 * DAY;
    const s: SeriesPointLike[] = [
      seriesPoint("2026-11-14", 400),
      seriesPoint("2026-11-18", 600),
    ];
    const c = buildSubscriptionCurve({
      series: s,
      quotaTiers: [{ name: "weekly_limit", utilization: 0, limit: null, resets_at: new Date(resetAt).toISOString() }],
      quotaQueriedAt: null,
      estTiers: [],
      nowMs: NOW,
    })!;
    expect(c.windowStart).toBe(windowStart);
    void start;
    const elapsedFrac = (NOW - windowStart) / (7 * DAY);
    const projected = 1000 / elapsedFrac;
    const expectedNowPct = 100 * (1 - 1000 / projected);
    expect(c.nowPct!).toBeCloseTo(expectedNowPct, 3);
    expect(c.calibrations.length).toBe(0);
    expect(c.maxDeviation).toBeNull();
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

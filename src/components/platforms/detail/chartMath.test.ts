// T3 图表数据映射测试（先红）：分档订阅曲线窗口/投影/耗尽判定、按量日柱、失败分类、模型分布。
import { describe, it, expect } from "vitest";
import {
  monotonePath,
  cycleMsForName,
  buildTierCurve,
  unitOf,
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
  it("five_hour → 5 小时；weekly → 7 天；month → 30 天；day → 24h；未知 → null", () => {
    expect(cycleMsForName("five_hour")).toBe(5 * HOUR);
    expect(cycleMsForName("weekly_limit")).toBe(7 * DAY);
    expect(cycleMsForName("mcp_monthly")).toBe(30 * DAY);
    expect(cycleMsForName("daily")).toBe(DAY);
    expect(cycleMsForName("whatever")).toBeNull();
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

describe("buildTierCurve", () => {
  const HOUR = 3_600_000;
  // 某档窗口：NOW-6h 开始，NOW+3h 重置（周期 9h，任意档均可）。
  const windowStart = NOW - 6 * HOUR;
  const resetAt = NOW + 3 * HOUR;

  it("分档构建：窗口外的点被排除，窗口内升序，snapshotPct = 最新点", () => {
    const c = buildTierCurve({
      windowStart, resetAt,
      points: [
        { t: windowStart - HOUR, pct: 100 },  // 窗口外（上轮残留）→ 排除
        { t: NOW - 3 * HOUR, pct: 80 },
        { t: NOW - 1 * HOUR, pct: 55 },
        { t: resetAt + HOUR, pct: 10 },       // 重置后 → 排除
      ],
      nowMs: NOW,
    })!;
    expect(c.actual.map(p => p.pct)).toEqual([80, 55]);
    expect(c.snapshotPct).toBe(55);
    expect(c.windowStart).toBe(windowStart);
    expect(c.resetAt).toBe(resetAt);
  });

  it("投影：按首末斜率外推，重置前仍有余量 → lasts", () => {
    // 100→80 历时 2h（-10%/h），重置在最新点后 4h → 剩 40% > 0。
    const lastT = NOW - 3 * HOUR;
    const c = buildTierCurve({
      windowStart, resetAt: lastT + 4 * HOUR,
      points: [
        { t: NOW - 5 * HOUR, pct: 100 },
        { t: NOW - 3 * HOUR, pct: 80 },
      ],
      nowMs: NOW,
    })!;
    expect(c.forecast).toEqual({ lasts: true, exhaustAt: null });
  });

  it("投影：斜率会提前归零 → lasts=false + exhaustAt = 最新点 + pct/速率", () => {
    // 100→80 历时 2h（-10%/h），80% 归零再需 8h；重置在最新点后 10h → 提前 2h 耗尽。
    const lastT = NOW - 3 * HOUR;
    const c = buildTierCurve({
      windowStart, resetAt: lastT + 10 * HOUR,
      points: [
        { t: NOW - 5 * HOUR, pct: 100 },
        { t: lastT, pct: 80 },
      ],
      nowMs: NOW,
    })!;
    expect(c.forecast!.lasts).toBe(false);
    expect(c.forecast!.exhaustAt).toBe(lastT + 8 * HOUR);
  });

  it("剩余上升或持平的速率 → lasts（无耗尽风险）", () => {
    const c = buildTierCurve({
      windowStart, resetAt,
      points: [{ t: NOW - 3 * HOUR, pct: 50 }, { t: NOW - HOUR, pct: 50 }],
      nowMs: NOW,
    })!;
    expect(c.forecast).toEqual({ lasts: true, exhaustAt: null });
  });

  it("轨迹点 <2 → 不投影（forecast null），快照仍显示", () => {
    const c = buildTierCurve({
      windowStart, resetAt,
      points: [{ t: NOW - HOUR, pct: 40 }],
      nowMs: NOW,
    })!;
    expect(c.actual).toHaveLength(1);
    expect(c.snapshotPct).toBe(40);
    expect(c.forecast).toBeNull();
  });

  it("无历史 → actual 空、fallback 快照兜底、不伪造轨迹", () => {
    const c = buildTierCurve({ windowStart, resetAt, points: [], fallbackPct: 33, nowMs: NOW })!;
    expect(c.actual).toEqual([]);
    expect(c.snapshotPct).toBe(33);
    expect(c.forecast).toBeNull();
  });

  it("窗口未知（无 resetAt）→ 仅 fallback 时返回退化曲线，否则 null", () => {
    expect(buildTierCurve({ windowStart: null, resetAt: null, points: [], fallbackPct: 20, nowMs: NOW }))
      .toEqual({ windowStart: 0, resetAt: 0, actual: [], snapshotPct: 20, forecast: null });
    expect(buildTierCurve({ windowStart: null, resetAt: null, points: [{ t: NOW, pct: 50 }], nowMs: NOW })).toBeNull();
  });

  it("窗口已过（resetAt ≤ now）→ 不投影", () => {
    const c = buildTierCurve({
      windowStart: NOW - 6 * HOUR, resetAt: NOW - HOUR,
      points: [{ t: NOW - 5 * HOUR, pct: 90 }, { t: NOW - 2 * HOUR, pct: 30 }],
      nowMs: NOW,
    })!;
    expect(c.actual).toHaveLength(2);
    expect(c.forecast).toBeNull();
  });

  it("unitOf：空/缺省按 tokens", () => {
    expect(unitOf(null)).toBe("tokens");
    expect(unitOf("prompt_count")).toBe("prompt_count");
    expect(unitOf("  ")).toBe("tokens");
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

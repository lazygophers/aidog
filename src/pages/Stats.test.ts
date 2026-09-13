// Stats 四 tab（#37 T7）纯函数单测：时间序列宽表合并 / 时刻热力聚合 / bucket 本地时区解析。
// 断言一律用 new Date(y, m, d, h, min) 同基准构造，不依赖跑测试机器的时区。
import { describe, it, expect } from "vitest";
import { bucketMs, buildTrendChartData, buildHeatCells } from "./Stats";
import type { StatsBucket, StatsSeries } from "../services/api";

function bucket(tb: string, requests: number): StatsBucket {
  return {
    time_bucket: tb,
    total_requests: requests,
    success_count: requests,
    error_count: 0,
    input_tokens: 0,
    output_tokens: 0,
    cache_tokens: 0,
    avg_duration_ms: 0,
    total_cost: 0,
  };
}
const series = (name: string, buckets: StatsBucket[]): StatsSeries => ({ name, buckets });

describe("bucketMs", () => {
  it("datetime 串按本地时区解析（空格补 T）", () => {
    expect(bucketMs("2026-09-13 14:30")).toBe(new Date(2026, 8, 13, 14, 30).getTime());
  });
  it("纯日期串补 T00:00:00 本地午夜（不被当 UTC）", () => {
    expect(bucketMs("2026-09-13")).toBe(new Date(2026, 8, 13).getTime());
  });
});

describe("buildTrendChartData", () => {
  it("series 空/单序列 → buckets 总量单序列（v）", () => {
    const buckets = [bucket("2026-09-13 10:00:00", 5), bucket("2026-09-13 11:00:00", 7)];
    for (const s of [[], [series("only", buckets)]]) {
      const r = buildTrendChartData(buckets, s, "requests");
      expect(r.multi).toBe(false);
      expect(r.config).toEqual({ v: { label: "requests" } });
      expect(r.rows).toEqual([
        { x: new Date(2026, 8, 13, 10).getTime(), v: 5 },
        { x: new Date(2026, 8, 13, 11).getTime(), v: 7 },
      ]);
    }
  });

  it("多序列 → 宽表合并 + 按总量降序（s0 = 最大维度，主琥珀线）", () => {
    const a = series("A", [bucket("2026-09-13 10:00:00", 1), bucket("2026-09-13 11:00:00", 2)]); // 总量 3
    const b = series("B", [bucket("2026-09-13 10:00:00", 10), bucket("2026-09-13 12:00:00", 20)]); // 总量 30
    const r = buildTrendChartData([], [a, b], "requests");
    expect(r.multi).toBe(true);
    expect(r.config).toEqual({ s0: { label: "B" }, s1: { label: "A" } });
    const t10 = new Date(2026, 8, 13, 10).getTime();
    const t11 = new Date(2026, 8, 13, 11).getTime();
    const t12 = new Date(2026, 8, 13, 12).getTime();
    // 三桶并集、x 升序；缺失格 undefined（Recharts 断线处理）
    expect(r.rows).toEqual([
      { x: t10, s0: 10, s1: 1 },
      { x: t11, s1: 2 },
      { x: t12, s0: 20 },
    ]);
  });
});

describe("buildHeatCells", () => {
  it("hourly 桶 → (星期, 小时) 聚合，同格累加", () => {
    const cells = buildHeatCells([
      bucket("2026-09-13 10:00:00", 3), // 2026-09-13 是周日
      bucket("2026-09-13 10:05:00", 4), // 同 (day=0, hour=10) 累加
      bucket("2026-09-14 09:00:00", 2), // 周一
    ]);
    expect(cells).toContainEqual({ day: 0, hour: 10, value: 7 });
    expect(cells).toContainEqual({ day: 1, hour: 9, value: 2 });
    expect(cells).toHaveLength(2);
  });
  it("daily 桶无小时信息不入格", () => {
    expect(buildHeatCells([bucket("2026-09-13", 99)])).toEqual([]);
  });
  it("脏时间串跳过不抛", () => {
    expect(buildHeatCells([bucket("bad bucket", 1)])).toEqual([]);
  });
});

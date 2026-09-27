// Home 命令面板（#36）纯函数单测：sparkline / 趋势线共用归一化。
// 三维度行列表（buildDimRows 维度无关，platform/model/group 共用）与今日窗口口径同此范式。
import { describe, it, expect } from "vitest";
import { normPoints, buildDimRows, todayStartMs, DIM_TOP_N } from "./Home";
import type { DimensionEntry } from "../services/api";

describe("todayStartMs（今日窗口口径）", () => {
  it("任意时刻 → 本地当日 00:00（与 tray todayStats 的本地 00:00 口径一致）", () => {
    expect(todayStartMs(new Date(2026, 8, 27, 15, 30, 12)))
      .toBe(new Date(2026, 8, 27, 0, 0, 0, 0).getTime());
  });

  it("凌晨边界 → 同日 00:00（不回退到昨天）", () => {
    expect(todayStartMs(new Date(2026, 8, 27, 0, 0, 0, 1)))
      .toBe(new Date(2026, 8, 27, 0, 0, 0, 0).getTime());
    expect(todayStartMs(new Date(2026, 8, 27, 23, 59, 59)))
      .toBe(new Date(2026, 8, 27, 0, 0, 0, 0).getTime());
  });
});

describe("normPoints", () => {
  it("空序列 → 空点集", () => {
    expect(normPoints([], 100, 22)).toEqual([]);
  });

  it("单点 → 居中", () => {
    expect(normPoints([5], 100, 22)).toEqual([[50, 11]]);
  });

  it("min-max 归一化：最大值顶 pad、最小值落 h-pad，x 均分覆盖全宽", () => {
    const pts = normPoints([0, 5, 10], 100, 22, 2);
    expect(pts[0]).toEqual([0, 20]);   // 最小值 → 底部
    expect(pts[1]).toEqual([50, 11]);  // 中值 → 中线
    expect(pts[2]).toEqual([100, 2]);  // 最大值 → 顶部
  });

  it("平坦序列 → 全部落中线（不除零）", () => {
    expect(normPoints([7, 7, 7], 90, 20, 2)).toEqual([[0, 10], [45, 10], [90, 10]]);
  });

  it("负值 / 小数序列同规则", () => {
    const pts = normPoints([-2, 2], 40, 10, 1);
    expect(pts[0][1]).toBeCloseTo(9);
    expect(pts[1][1]).toBeCloseTo(1);
  });
});

const dim = (name: string, tokens: number, req = 0, success = 0, cost = 0): DimensionEntry => ({
  name,
  total_requests: req,
  success_count: success,
  // tokens 拆三份放不改变合计，构造简单。
  input_tokens: tokens,
  output_tokens: 0,
  cache_tokens: 0,
  cache_rate: 0,
  avg_duration_ms: 0,
  total_cost: cost,
});

describe("buildDimRows（home-model-stats）", () => {
  it("空数据 → 零行、total 0", () => {
    const r = buildDimRows([]);
    expect(r.rows).toEqual([]);
    expect(r.total).toBe(0);
  });

  it("不足 TopN → 全保留、无「其它」行", () => {
    const r = buildDimRows([dim("a", 100), dim("b", 50)]);
    expect(r.rows.map(x => x.name)).toEqual(["a", "b"]);
    expect(r.rows.every(x => !x.other)).toBe(true);
    expect(r.total).toBe(150);
  });

  it("超过 TopN → tokens 降序截断 + 「其它」行合并剩余", () => {
    const data = Array.from({ length: DIM_TOP_N + 2 }, (_, i) => dim(`m${i}`, 1000 - i * 100, 10, 8, 1));
    const r = buildDimRows(data);
    expect(r.rows).toHaveLength(DIM_TOP_N + 1);
    expect(r.rows[0].name).toBe("m0");
    expect(r.rows[DIM_TOP_N - 1].name).toBe(`m${DIM_TOP_N - 1}`);
    const other = r.rows[DIM_TOP_N];
    expect(other.other).toBe(true);
    // 合并行 = m8 + m9 的 tokens/cost/requests 求和。
    expect(other.d.input_tokens).toBe(200 + 100);
    expect(other.d.total_cost).toBe(2);
    expect(other.d.total_requests).toBe(20);
    expect(r.total).toBe(data.reduce((s, d) => s + d.input_tokens, 0));
  });

  it("排序按 tokens 不按 requests（后端 dimension_data 按 requests 排）", () => {
    const r = buildDimRows([dim("many-req", 10, 999), dim("few-req", 1000, 1)]);
    expect(r.rows[0].name).toBe("few-req");
  });
});

describe("buildDimRows 空名维度（未知平台修复）", () => {
  it("模型维度：空名行滤掉（防旧内核未过滤的数据）", () => {
    const r = buildDimRows([dim("", 999), dim("m1", 100)]);
    expect(r.rows.map(x => x.name)).toEqual(["m1"]);
    expect(r.total).toBe(100);
  });

  it("分组维度：空名标「未分组平台」且不可点（unclickable）", () => {
    const r = buildDimRows([dim("g1", 500), dim("", 300)], "未分组平台");
    const names = r.rows.map(x => x.name);
    expect(names).toContain("未分组平台");
    const ungrouped = r.rows.find(x => x.name === "未分组平台")!;
    expect(ungrouped.unclickable).toBe(true);
    expect(ungrouped.other).toBe(false); // label 显示自己的名字，不是「其它」
    expect(ungrouped.d.input_tokens).toBe(300);
    expect(r.total).toBe(800);
  });
});

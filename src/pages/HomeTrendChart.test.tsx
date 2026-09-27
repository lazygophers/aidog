// 维度趋势区块（home-dim-trend）单测：纯函数（buildDimTrend / buildSparkMap）+ 组件切换。
// Home.test.ts 同范式；组件测试走共享 render（空 resources i18n，断言用 key）。
import { describe, it, expect } from "vitest";
import { render, screen, fireEvent } from "../test/render";
import { buildDimTrend, buildSparkMap, HomeTrendChart } from "./HomeTrendChart";
import { DIM_TOP_N } from "./Home";
import type { StatsBucket, StatsSeries } from "../services/api";

const bucket = (hour: string, req: number, tokens: number, cost: number): StatsBucket => ({
  time_bucket: `2026-09-27 ${hour}:00:00`,
  total_requests: req,
  success_count: req,
  error_count: 0,
  input_tokens: tokens,
  output_tokens: 0,
  cache_tokens: 0,
  avg_duration_ms: 0,
  total_cost: cost,
});

const series = (name: string, buckets: StatsBucket[]): StatsSeries => ({ name, buckets });

describe("buildDimTrend（home-dim-trend）", () => {
  const sBig = series("big", [bucket("10", 5, 1000, 1.5), bucket("11", 3, 500, 0.5)]);
  const sSmall = series("small", [bucket("10", 1, 100, 0.1)]);

  it("cost 指标：宽表逐桶 total_cost，req = 全维度请求合计", () => {
    const { config, rows } = buildDimTrend([sBig, sSmall], "cost", { other: "其它" });
    expect(Object.keys(config)).toEqual(["s0", "s1"]);
    expect(config.s0.label).toBe("big");
    expect(config.s1.label).toBe("small");
    expect(rows).toHaveLength(2); // 10 点 + 11 点两个桶（time_bucket 升序）
    const h10 = rows[0];
    expect(h10.s0).toBe(1.5);
    expect(h10.s1).toBe(0.1);
    expect(h10.req).toBe(6); // 5 + 1
    expect(rows[1].req).toBe(3);
  });

  it("tokens 指标：值换三 token 合计，行集合不变", () => {
    const { rows } = buildDimTrend([sBig, sSmall], "tokens", { other: "其它" });
    expect(rows[0].s0).toBe(1000);
    expect(rows[0].s1).toBe(100);
    expect(rows[0].req).toBe(6);
  });

  it("超过 TopN → tokens 降序截断 + 「其它」合并剩余（与 buildDimRows 口径一致）", () => {
    const many = Array.from({ length: DIM_TOP_N + 2 }, (_, i) =>
      series(`m${i}`, [bucket("10", 1, 1000 - i * 100, 1)]),
    );
    const { config, rows } = buildDimTrend(many, "tokens", { other: "其它" });
    expect(Object.keys(config)).toHaveLength(DIM_TOP_N + 1);
    expect(config.s0.label).toBe("m0");
    expect(config[`s${DIM_TOP_N}`].label).toBe("其它");
    // 「其它」= m8 + m9 的 tokens 合计
    expect(rows[0][`s${DIM_TOP_N}`]).toBe(200 + 100);
    // 排序按 tokens 不按请求数
    const byReq = [series("many-req", [bucket("10", 99, 10, 0)]), series("few-req", [bucket("10", 1, 1000, 0)])];
    expect(buildDimTrend(byReq, "tokens", { other: "其它" }).config.s0.label).toBe("few-req");
  });

  it("分组维度：空名序列标签归 ungroupedLabel", () => {
    const { config } = buildDimTrend([series("", [bucket("10", 1, 5, 0)])], "tokens", {
      other: "其它",
      ungrouped: "未分组平台",
    });
    expect(config.s0.label).toBe("未分组平台");
  });

  it("空序列 → 零行宽表（组件落诚实空态）", () => {
    const { config, rows } = buildDimTrend([], "cost", { other: "其它" });
    expect(rows).toEqual([]);
    expect(Object.keys(config)).toEqual([]);
  });
});

describe("buildSparkMap（行迷你曲线）", () => {
  it("key = 维度名，值 = 逐桶 token 序列（桶序即后端升序）", () => {
    const m = buildSparkMap([series("a", [bucket("10", 1, 3, 0), bucket("11", 1, 5, 0)])]);
    expect(m.get("a")).toEqual([3, 5]);
  });

  it("分组维度空名归 ungroupedLabel（与 buildDimRows 改名一致）", () => {
    const m = buildSparkMap([series("", [bucket("10", 1, 7, 0)])], "未分组平台");
    expect(m.get("未分组平台")).toEqual([7]);
  });
});

describe("HomeTrendChart（组件）", () => {
  const props = {
    modelSeries: [series("m0", [bucket("10", 2, 100, 0.3), bucket("11", 1, 50, 0.1)])],
    groupSeries: [series("g0", [bucket("10", 9, 900, 2)])],
    ungroupedLabel: "未分组平台",
    loading: false,
  };

  it("默认按模型 + 花费模式渲染堆叠面积与右轴请求线", () => {
    const { container } = render(<HomeTrendChart {...props} />);
    expect(container.querySelectorAll(".recharts-area").length).toBe(1);
    expect(container.querySelectorAll(".recharts-line").length).toBe(1); // 右轴请求线
    expect(screen.getByText("home.tabModel")).toBeTruthy();
    expect(screen.getByText("home.trendCost").closest("button")?.getAttribute("aria-pressed")).toBe("true");
  });

  it("切到按分组 → 消费 groupSeries 数据", () => {
    const { container } = render(<HomeTrendChart {...props} />);
    fireEvent.click(screen.getByText("home.tabGroup"));
    // g0 两桶 token 900 / 0 → 唯一层的值来自分组序列（tooltip label 进 legend）
    expect(container.querySelector(".recharts-legend-wrapper")?.textContent).toContain("g0");
  });

  it("切到 Token 指标 → aria-pressed 跟随", () => {
    render(<HomeTrendChart {...props} />);
    fireEvent.click(screen.getByText("home.tokens"));
    expect(screen.getByText("home.tokens").closest("button")?.getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByText("home.trendCost").closest("button")?.getAttribute("aria-pressed")).toBe("false");
  });

  it("空数据 → 诚实空态（charts.noData），tab 仍可切", () => {
    render(<HomeTrendChart modelSeries={[]} groupSeries={[]} ungroupedLabel="u" loading={false} />);
    expect(screen.getByText("charts.noData")).toBeTruthy();
    expect(screen.getByRole("button", { name: "home.tabGroup" })).toBeTruthy();
  });
});

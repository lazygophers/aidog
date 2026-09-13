import { describe, it, expect } from "vitest";
import { render, screen } from "../../../test/render";
import { StackedAreaChart } from "../StackedAreaChart";

const T0 = Date.parse("2026-09-13T00:00:00");
const DAY = 86_400_000;

function rows(n: number): Record<string, unknown>[] {
  return Array.from({ length: n }, (_, i) => ({
    x: T0 + i * DAY,
    claude: (i % 5) + 1,
    glm: (i % 3) + 1,
  }));
}

const config = { claude: { label: "claude" }, glm: { label: "glm" } };

describe("StackedAreaChart", () => {
  it("renders one stacked area layer per series on a time axis", () => {
    const { container } = render(<StackedAreaChart config={config} data={rows(14)} />);
    expect(container.querySelector("svg")).toBeTruthy();
    // 两个系列 → 两个 Area 层（同一 stackId）
    expect(container.querySelectorAll(".recharts-area").length).toBe(2);
    // 时间轴刻度文本（14 天跨度 → MM-DD 标签）
    expect(container.querySelectorAll(".recharts-cartesian-axis-tick-value").length).toBeGreaterThan(0);
  });

  it("renders honest empty state for empty data", () => {
    render(<StackedAreaChart config={config} data={[]} />);
    expect(screen.getByText("charts.noData")).toBeTruthy();
  });

  it("renders legend with config labels for multi-series, none for single", () => {
    const multi = render(<StackedAreaChart config={config} data={rows(14)} />);
    const legend = multi.container.querySelector(".recharts-legend-wrapper");
    expect(legend?.textContent).toContain("claude");
    expect(legend?.textContent).toContain("glm");

    const single = render(
      <StackedAreaChart config={{ claude: { label: "claude" } }} data={rows(14)} />,
    );
    expect(single.container.querySelector(".recharts-legend-wrapper")).toBeNull();
  });

  it("scales Y axis to stacked totals, not single-series max", () => {
    // 单系列 max = 5，堆叠总量 max = 7；Y 顶刻度须 ≥ 7（nice-ticks 0 起步）。
    // X 轴是 MM-DD 时间标签（非数值）→ 过滤 NaN 只留 Y 数值刻度。
    const { container } = render(<StackedAreaChart config={config} data={rows(14)} tickCount={4} />);
    const ticks = Array.from(container.querySelectorAll(".recharts-cartesian-axis-tick-value"))
      .map((el) => Number(el.textContent))
      .filter(Number.isFinite);
    expect(ticks.length).toBeGreaterThan(0);
    expect(Math.max(...ticks)).toBeGreaterThanOrEqual(7);
  });

  it("downsamples >500 points and notes it in subtitle", () => {
    const { container } = render(<StackedAreaChart title="t" config={config} data={rows(600)} />);
    expect(screen.getByText(/charts\.downsampled/)).toBeTruthy();
    expect(container.querySelector("svg")).toBeTruthy();
  });

  it("formats Y axis ticks through valueFormat", () => {
    const { container } = render(
      <StackedAreaChart config={config} data={rows(14)} valueFormat={(n) => `$${n.toFixed(1)}`} />,
    );
    const texts = Array.from(container.querySelectorAll(".recharts-cartesian-axis-tick-value")).map(
      (el) => el.textContent,
    );
    expect(texts.some((tx) => tx != null && tx.includes("$"))).toBe(true);
  });
});

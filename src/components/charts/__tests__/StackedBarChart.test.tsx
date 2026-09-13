import { describe, it, expect } from "vitest";
import { render, screen } from "../../../test/render";
import { StackedBarChart } from "../StackedBarChart";

const data = [
  { name: "09-10", claude: 5, glm: 2 },
  { name: "09-11", claude: 3, glm: 4 },
  { name: "09-12", claude: 6, glm: 1 },
];

const config = { claude: { label: "claude" }, glm: { label: "glm" } };

describe("StackedBarChart", () => {
  it("renders stacked bar segments per series on a category axis", () => {
    const { container } = render(<StackedBarChart config={config} data={data} />);
    expect(container.querySelector("svg")).toBeTruthy();
    // 两个系列 → 两组柱
    expect(container.querySelectorAll(".recharts-bar").length).toBe(2);
    expect(container.querySelectorAll(".recharts-bar-rectangle").length).toBe(6);
    // 类目名上轴
    expect(screen.getByText("09-10")).toBeTruthy();
  });

  it("renders honest empty state for empty data", () => {
    render(<StackedBarChart config={config} data={[]} />);
    expect(screen.getByText("charts.noData")).toBeTruthy();
  });

  it("scales Y axis to stacked totals, not single-series max", () => {
    // 单系列 max = 6，堆叠总量 max = 7；Y 顶刻度须 ≥ 7。
    // X 轴是类目名（非数值）→ 过滤 NaN 只留 Y 数值刻度。
    const { container } = render(<StackedBarChart config={config} data={data} tickCount={4} />);
    const ticks = Array.from(container.querySelectorAll(".recharts-cartesian-axis-tick-value"))
      .map((el) => Number(el.textContent))
      .filter(Number.isFinite);
    expect(ticks.length).toBeGreaterThan(0);
    expect(Math.max(...ticks)).toBeGreaterThanOrEqual(7);
  });

  it("renders legend with config labels for multi-series, none for single", () => {
    const multi = render(<StackedBarChart config={config} data={data} />);
    const legend = multi.container.querySelector(".recharts-legend-wrapper");
    expect(legend?.textContent).toContain("claude");
    expect(legend?.textContent).toContain("glm");

    const single = render(
      <StackedBarChart config={{ claude: { label: "claude" } }} data={data} />,
    );
    expect(single.container.querySelector(".recharts-legend-wrapper")).toBeNull();
  });

  it("downsamples >500 categories and notes it in subtitle", () => {
    const many = Array.from({ length: 600 }, (_, i) => ({ name: `d${i}`, claude: i % 7, glm: i % 3 }));
    const { container } = render(<StackedBarChart title="t" config={config} data={many} />);
    expect(screen.getByText(/charts\.downsampled/)).toBeTruthy();
    expect(container.querySelector("svg")).toBeTruthy();
  });

  it("formats Y axis ticks through valueFormat", () => {
    const { container } = render(
      <StackedBarChart config={config} data={data} valueFormat={(n) => `$${n.toFixed(1)}`} />,
    );
    const texts = Array.from(container.querySelectorAll(".recharts-cartesian-axis-tick-value")).map(
      (el) => el.textContent,
    );
    expect(texts.some((tx) => tx != null && tx.includes("$"))).toBe(true);
  });
});

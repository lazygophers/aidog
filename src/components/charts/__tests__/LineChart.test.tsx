import { describe, it, expect } from "vitest";
import { render, screen } from "../../../test/render";
import { LineChart } from "../LineChart";

const T0 = Date.parse("2026-09-13T00:00:00");
const HOUR = 3_600_000;

function rows(n: number): Record<string, unknown>[] {
  return Array.from({ length: n }, (_, i) => ({ x: T0 + i * HOUR, cost: (i % 7) + 0.5, tokens: i * 10 }));
}

const config = {
  cost: { label: "cost" },
  tokens: { label: "tokens" },
};

describe("LineChart", () => {
  it("renders axes and series paths for normal data", () => {
    const { container } = render(<LineChart config={config} data={rows(24)} />);
    expect(container.querySelector("svg")).toBeTruthy();
    // 两个系列各至少一条折线 path（recharts Line 渲染 curve path）
    expect(container.querySelectorAll(".recharts-line-curve").length).toBe(2);
    // Y 轴 nice-ticks 刻度文本存在
    expect(container.querySelectorAll(".recharts-cartesian-axis-tick-value").length).toBeGreaterThan(0);
  });

  it("renders honest empty state for empty data", () => {
    render(<LineChart config={config} data={[]} />);
    expect(screen.getByText("charts.noData")).toBeTruthy();
  });

  it("downsamples >500 points and notes it in subtitle", () => {
    const { container } = render(<LineChart title="t" config={config} data={rows(600)} />);
    // 600 点 → LTTB 降到 500，副题出现降采样提示 key（测试 i18n 回传 key 本身）
    expect(screen.getByText(/charts\.downsampled/)).toBeTruthy();
    expect(container.querySelector("svg")).toBeTruthy();
  });

  it("renders legend with config labels for multi-series, none for single", () => {
    const multi = render(<LineChart config={config} data={rows(24)} />);
    const legend = multi.container.querySelector(".recharts-legend-wrapper");
    expect(legend).toBeTruthy();
    // legend 标签走 config label（内部键 cost/tokens 不外露）
    expect(legend?.textContent).toContain("cost");
    expect(legend?.textContent).toContain("tokens");

    const single = render(<LineChart config={{ cost: { label: "cost" } }} data={rows(12)} />);
    expect(single.container.querySelector(".recharts-legend-wrapper")).toBeNull();
  });

  it("formats Y axis ticks through valueFormat", () => {
    const { container } = render(
      <LineChart config={{ cost: { label: "cost" } }} data={rows(12)} valueFormat={(n) => `$${n.toFixed(2)}`} />,
    );
    const texts = Array.from(container.querySelectorAll(".recharts-cartesian-axis-tick-value")).map(
      (el) => el.textContent,
    );
    expect(texts.some((tx) => tx != null && tx.includes("$"))).toBe(true);
  });
});

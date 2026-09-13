import { describe, it, expect } from "vitest";
import { render, screen } from "../../../test/render";
import { BarChart } from "../BarChart";

const data = [
  { name: "claude", cost: 1.5, tokens: 120 },
  { name: "gpt", cost: 0.8, tokens: 90 },
  { name: "glm", cost: 2.2, tokens: 200 },
];

const config = { cost: { label: "cost" }, tokens: { label: "tokens" } };

describe("BarChart", () => {
  it("renders category axis and bar rectangles per series", () => {
    const { container } = render(<BarChart config={config} data={data} />);
    expect(container.querySelector("svg")).toBeTruthy();
    // 两个系列 → 两组柱
    expect(container.querySelectorAll(".recharts-bar").length).toBe(2);
    expect(container.querySelectorAll(".recharts-bar-rectangle").length).toBe(6);
    // 类目名上轴
    expect(screen.getByText("claude")).toBeTruthy();
  });

  it("renders honest empty state for empty data", () => {
    render(<BarChart config={config} data={[]} />);
    expect(screen.getByText("charts.noData")).toBeTruthy();
  });

  it("formats Y axis ticks through valueFormat", () => {
    const { container } = render(
      <BarChart config={config} data={data} valueFormat={(n) => `$${n.toFixed(1)}`} />,
    );
    const texts = Array.from(container.querySelectorAll(".recharts-cartesian-axis-tick-value")).map(
      (el) => el.textContent,
    );
    expect(texts.some((tx) => tx != null && tx.includes("$"))).toBe(true);
  });
});

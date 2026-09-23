import { describe, it, expect } from "vitest";
import { render, screen } from "../../../test/render";
import { DonutChart } from "../DonutChart";

function entries(n: number): { name: string; value: number }[] {
  return Array.from({ length: n }, (_, i) => ({ name: `p${i + 1}`, value: n - i }));
}

describe("DonutChart", () => {
  it("merges beyond topN into a single rest slice with legend rows", () => {
    render(<DonutChart data={entries(6)} topN={4} />);
    // top4 + 「其他」= 5 行图例；其他行文案走既有 stats.donutRest key（测试 i18n 回传 key）
    expect(screen.getByText("stats.donutRest")).toBeTruthy();
    for (const name of ["p1", "p2", "p3", "p4"]) expect(screen.getByText(name)).toBeTruthy();
    expect(screen.queryByText("p5")).toBeNull();
  });

  it("shows center total formatted and center label", () => {
    render(
      <DonutChart data={entries(3)} formatValue={(n) => `$${n.toFixed(2)}`} centerLabel="total" />,
    );
    expect(screen.getByText("$6.00")).toBeTruthy(); // 3+2+1
    expect(screen.getByText("total")).toBeTruthy();
  });

  it("renders honest empty state for zero / single effective slice", () => {
    const { rerender } = render(<DonutChart data={[]} />);
    expect(screen.getByText("charts.noData")).toBeTruthy();
    rerender(<DonutChart data={[{ name: "only", value: 5 }]} />);
    expect(screen.getByText("charts.noData")).toBeTruthy();
  });

  it("first slice amber, rest grayscale", () => {
    const { container } = render(<DonutChart data={entries(3)} animate={false} />);
    const fills = Array.from(container.querySelectorAll(".recharts-pie-sector path")).map((el) =>
      el.getAttribute("fill"),
    );
    // 主系列钉的是 data-primary 不是 primary：2026-09-23 深色强调色改成近黑之后，
    // 「图表跟着界面填充色走」这条被推翻了（近黑折线画在卡片上 1.01:1）。
    expect(fills[0]).toBe("var(--data-primary)");
    expect(fills[1]).toBe("var(--chart-2)");
    expect(fills[2]).toBe("var(--chart-3)");
  });

  it("mini renders bare with legend rows; showLegend=false hides the side list", () => {
    const { container, rerender } = render(<DonutChart mini animate={false} data={entries(3)} />);
    // 裸渲染：无 ChartCard 壳；图例行仍在
    expect(container.querySelector(".glass-surface")).toBeNull();
    expect(screen.getByText("p1")).toBeTruthy();

    rerender(<DonutChart mini animate={false} data={entries(3)} showLegend={false} />);
    expect(screen.queryByText("p1")).toBeNull();
  });

  it("mini returns null instead of the empty-state card when under two slices", () => {
    const { container } = render(<DonutChart mini animate={false} data={[{ name: "only", value: 5 }]} />);
    expect(container.innerHTML).toBe("");
  });
});

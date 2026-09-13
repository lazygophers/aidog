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
    expect(fills[0]).toBe("var(--primary)");
    expect(fills[1]).toBe("var(--chart-2)");
    expect(fills[2]).toBe("var(--chart-3)");
  });
});

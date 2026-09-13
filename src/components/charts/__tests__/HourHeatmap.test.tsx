import { describe, it, expect } from "vitest";
import { render, screen, within } from "../../../test/render";
import { HourHeatmap } from "../HourHeatmap";

describe("HourHeatmap", () => {
  it("renders a 24×7 grid (168 cells) with hour axis labels", () => {
    render(<HourHeatmap data={[{ day: 1, hour: 10, value: 3 }]} />);
    const grid = screen.getByRole("img");
    const cells = grid.querySelectorAll("[data-cell]");
    expect(cells.length).toBe(168);
    // 顶部小时刻度 00/06/12/18
    for (const h of ["00", "06", "12", "18"]) expect(within(grid).getByText(h)).toBeTruthy();
  });

  it("renders honest empty state for empty data", () => {
    render(<HourHeatmap data={[]} />);
    expect(screen.getByText("charts.noData")).toBeTruthy();
  });

  it("maps value intensity through the amber heat band", () => {
    render(
      <HourHeatmap
        data={[
          { day: 2, hour: 9, value: 1 },
          { day: 2, hour: 14, value: 10 },
        ]}
      />,
    );
    const grid = screen.getByRole("img");
    const cold = grid.querySelector('[data-cell="2-9"]')!.getAttribute("style")!;
    const hot = grid.querySelector('[data-cell="2-14"]')!.getAttribute("style")!;
    const alpha = (s: string) => Number(s.match(/rgba\(232, 197, 71, ([\d.]+)\)/)![1]);
    expect(alpha(cold)).toBeLessThan(alpha(hot));
    // 满值格打到色带顶
    expect(alpha(hot)).toBeCloseTo(0.92, 2);
  });

  it("falls back to the lowest band when all values are zero", () => {
    const { container } = render(<HourHeatmap data={[{ day: 0, hour: 0, value: 0 }]} />);
    const cell = container.querySelector('[data-cell="0-0"]')!.getAttribute("style")!;
    expect(cell).toContain("rgba(232, 197, 71, 0.06)");
  });
});

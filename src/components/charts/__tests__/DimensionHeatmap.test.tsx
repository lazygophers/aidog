import { describe, it, expect } from "vitest";
import { render, screen } from "../../../test/render";
import { DimensionHeatmap } from "../DimensionHeatmap";

const DAY0 = Date.parse("2026-09-10T00:00:00");
const DAY = 86_400_000;
const d = (i: number) => DAY0 + i * DAY;

describe("DimensionHeatmap", () => {
  it("renders rows × columns grid with day axis labels", () => {
    render(
      <DimensionHeatmap
        data={[
          { name: "claude", day: d(0), value: 1 },
          { name: "claude", day: d(1), value: 2 },
          { name: "glm", day: d(0), value: 3 },
        ]}
      />,
    );
    const grid = screen.getByRole("img");
    // 2 维度行 × 2 日列 = 4 格
    expect(grid.querySelectorAll("[data-cell]").length).toBe(4);
    // 维度行名与列刻度（2 列 ≤ 10 → 每列都标）
    expect(screen.getByText("claude")).toBeTruthy();
    expect(screen.getByText("glm")).toBeTruthy();
    expect(screen.getByText("09-10")).toBeTruthy();
    expect(screen.getByText("09-11")).toBeTruthy();
  });

  it("renders honest empty state for empty data", () => {
    render(<DimensionHeatmap data={[]} />);
    expect(screen.getByText("charts.noData")).toBeTruthy();
  });

  it("maps value intensity through the amber heat band, zero cells to floor", () => {
    render(
      <DimensionHeatmap
        data={[
          { name: "claude", day: d(0), value: 1 },
          { name: "claude", day: d(1), value: 10 },
          { name: "glm", day: d(0), value: 0 },
          { name: "glm", day: d(1), value: 0 },
        ]}
      />,
    );
    const grid = screen.getByRole("img");
    const alpha = (sel: string) =>
      Number(
        grid
          .querySelector(sel)!
          .getAttribute("style")!
          .match(/rgba\(232, 197, 71, ([\d.]+)\)/)![1],
      );
    expect(alpha('[data-cell="claude|' + d(0) + '"]')).toBeLessThan(alpha('[data-cell="claude|' + d(1) + '"]'));
    // 满值格打到色带顶
    expect(alpha('[data-cell="claude|' + d(1) + '"]')).toBeCloseTo(0.92, 2);
    // 缺格/零格回落色带最低档
    expect(alpha('[data-cell="glm|' + d(1) + '"]')).toBeCloseTo(0.06, 3);
  });

  it("custom formatDay and formatValue land in cell title tooltip", () => {
    render(
      <DimensionHeatmap
        data={[{ name: "claude", day: d(0), value: 7 }]}
        formatDay={(ms) => `day-${new Date(ms).getDate()}`}
        formatValue={(n) => `${n} req`}
      />,
    );
    const cell = screen.getByRole("img").querySelector("[data-cell]")!;
    expect(cell.getAttribute("title")).toBe("claude day-10 · 7 req");
  });
});

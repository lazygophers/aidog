import { describe, it, expect } from "vitest";
import { render, screen } from "../../../test/render";
import { HourHeatBar } from "../HourHeatBar";

describe("HourHeatBar", () => {
  it("renders exactly 24 cells keyed by hour, missing hours as 0", () => {
    render(<HourHeatBar data={[{ hour: 9, value: 5 }]} />);
    const bar = screen.getByRole("img");
    const cells = bar.querySelectorAll("[data-heat-hour]");
    expect(cells.length).toBe(24);
    // 缺省小时走色带最低档（值 0）
    expect(cells[0]!.getAttribute("style")).toContain("rgba(232, 197, 71, 0.06)");
  });

  it("maps value intensity through the amber heat band, max cell at top", () => {
    render(<HourHeatBar data={[{ hour: 3, value: 1 }, { hour: 20, value: 10 }]} />);
    const bar = screen.getByRole("img");
    const cold = bar.querySelector('[data-heat-hour="3"]')!.getAttribute("style")!;
    const hot = bar.querySelector('[data-heat-hour="20"]')!.getAttribute("style")!;
    const alpha = (s: string) => Number(s.match(/rgba\(232, 197, 71, ([\d.]+)\)/)![1]);
    expect(alpha(cold)).toBeLessThan(alpha(hot));
    expect(alpha(hot)).toBeCloseTo(0.92, 2);
  });

  it("formats per-cell title through formatValue", () => {
    render(
      <HourHeatBar data={[{ hour: 14, value: 1234 }]} formatValue={(n) => `${n}r`} ariaLabel="heat" />,
    );
    expect(screen.getByRole("img", { name: "heat" }).querySelector('[data-heat-hour="14"]')!.getAttribute("title")).toBe("14:00 · 1234r");
  });

  it("ignores out-of-range hours instead of crashing", () => {
    render(<HourHeatBar data={[{ hour: 25, value: 9 }, { hour: -1, value: 9 }]} />);
    expect(screen.getByRole("img").querySelectorAll("[data-heat-hour]").length).toBe(24);
  });
});

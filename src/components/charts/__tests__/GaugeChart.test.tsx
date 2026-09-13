import { describe, it, expect } from "vitest";
import { render, screen } from "../../../test/render";
import { GaugeChart } from "../GaugeChart";

describe("GaugeChart", () => {
  it("renders a meter with value attributes and center readouts", () => {
    render(
      <GaugeChart value={30} max={120} formatValue={(n) => `${n}h`} label="glm" />,
    );
    const meter = screen.getByRole("meter");
    expect(meter.getAttribute("aria-valuemin")).toBe("0");
    expect(meter.getAttribute("aria-valuemax")).toBe("120");
    expect(meter.getAttribute("aria-valuenow")).toBe("30");
    // 中央：25% 百分比（整数位）+ 格式化值
    expect(screen.getByText("25%")).toBeTruthy();
    expect(screen.getByText("30h")).toBeTruthy();
    expect(screen.getByText("glm")).toBeTruthy();
  });

  it("renders honest empty state when max is not positive", () => {
    render(<GaugeChart value={5} max={0} />);
    expect(screen.getByText("charts.noData")).toBeTruthy();
  });

  it("maps fraction to arc angle and clamps overflow", () => {
    const { rerender } = render(<GaugeChart value={50} max={100} />);
    const bgOf = () => {
      const meter = screen.getByRole("meter");
      return (meter.firstElementChild as HTMLElement).style.background;
    };
    // 0.5 → 180deg 琥珀弧；满配额 → 360deg
    expect(bgOf()).toContain("180deg");
    rerender(<GaugeChart value={999} max={100} />);
    expect(bgOf()).toContain("360deg");
  });

  it("renders trend sparkline polyline for multi-point series, dot for single", () => {
    const { container, rerender } = render(
      <GaugeChart value={3} max={10} trend={[{ at: 0, fraction: 1 }, { at: 60, fraction: 0.3 }]} />,
    );
    const spark = () => container.querySelector('[data-testid="gauge-trend"]');
    expect(spark()?.querySelector("polyline")).toBeTruthy();
    expect(spark()?.querySelector("circle")).toBeNull();

    rerender(<GaugeChart value={3} max={10} trend={[{ at: 0, fraction: 0.3 }]} />);
    expect(spark()?.querySelector("polyline")).toBeNull();
    expect(spark()?.querySelector("circle")).toBeTruthy();

    // 无 trend → 不画 sparkline
    rerender(<GaugeChart value={3} max={10} />);
    expect(spark()).toBeNull();
  });
});

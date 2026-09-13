// @vitest-environment node
import { describe, it, expect } from "vitest";
import { PRIMARY_COLOR, seriesColor, seriesColors, heatColor, HEAT_MIN_ALPHA, HEAT_MAX_ALPHA } from "../palette";

describe("seriesColor", () => {
  it("index 0 and negative → primary amber", () => {
    expect(seriesColor(0)).toBe("var(--primary)");
    expect(seriesColor(-3)).toBe(PRIMARY_COLOR);
  });

  it("index 1..4 map to aux greys in order", () => {
    expect(seriesColor(1)).toBe("var(--chart-2)");
    expect(seriesColor(2)).toBe("var(--chart-3)");
    expect(seriesColor(3)).toBe("var(--chart-4)");
    expect(seriesColor(4)).toBe("var(--chart-5)");
  });

  it("index beyond 4 wraps around the aux cycle", () => {
    expect(seriesColor(5)).toBe("var(--chart-2)");
    expect(seriesColor(8)).toBe("var(--chart-5)");
    expect(seriesColor(9)).toBe("var(--chart-2)");
  });
});

describe("seriesColors", () => {
  it("returns count colors following the same mapping", () => {
    expect(seriesColors(6)).toEqual([
      "var(--primary)",
      "var(--chart-2)",
      "var(--chart-3)",
      "var(--chart-4)",
      "var(--chart-5)",
      "var(--chart-2)",
    ]);
  });

  it("zero count → empty array", () => {
    expect(seriesColors(0)).toEqual([]);
  });
});

describe("heatColor", () => {
  it("t=0 → min alpha, t=1 → max alpha", () => {
    expect(heatColor(0)).toBe(`rgba(232, 197, 71, ${HEAT_MIN_ALPHA.toFixed(3)})`);
    expect(heatColor(1)).toBe(`rgba(232, 197, 71, ${HEAT_MAX_ALPHA.toFixed(3)})`);
  });

  it("t between endpoints interpolates alpha", () => {
    const mid = HEAT_MIN_ALPHA + (HEAT_MAX_ALPHA - HEAT_MIN_ALPHA) * 0.5;
    expect(heatColor(0.5)).toBe(`rgba(232, 197, 71, ${mid.toFixed(3)})`);
  });

  it("t outside [0,1] clamps to endpoints", () => {
    expect(heatColor(-1)).toBe(heatColor(0));
    expect(heatColor(2)).toBe(heatColor(1));
  });
});

// @vitest-environment node
import { describe, it, expect } from "vitest";
import {
  formatNumber,
  formatCost,
  formatCostUsd,
  formatPercent,
  successRate,
  sumTokens,
} from "./formatters";

describe("formatNumber", () => {
  it("abbreviates millions with 1 decimal", () => {
    expect(formatNumber(1_200_000)).toBe("1.2M");
    expect(formatNumber(1_000_000)).toBe("1.0M");
  });
  it("abbreviates thousands with 1 decimal", () => {
    expect(formatNumber(3_500)).toBe("3.5K");
    expect(formatNumber(1_000)).toBe("1.0K");
  });
  it("formats integers below 1000 without decimals", () => {
    expect(formatNumber(999)).toBe("999");
    expect(formatNumber(0)).toBe("0");
  });
  it("formats non-integers below 1000 with 1 decimal", () => {
    expect(formatNumber(12.34)).toBe("12.3");
  });
});

describe("formatCost", () => {
  it("returns 0 for non-positive / NaN", () => {
    expect(formatCost(0)).toBe("0");
    expect(formatCost(-5)).toBe("0");
    expect(formatCost(NaN)).toBe("0");
  });
  it("uses 2 decimals for >= 1", () => {
    expect(formatCost(12.345)).toBe("12.35");
    expect(formatCost(1)).toBe("1.00");
  });
  it("uses 3 decimals for >= 0.01", () => {
    expect(formatCost(0.0345)).toBe("0.035");
    expect(formatCost(0.01)).toBe("0.010");
  });
  it("renders tiny non-zero costs as fixed decimals, never rounding to 0", () => {
    expect(formatCost(0.0034)).toBe("0.00340");
    // 4.5e-7 → 不被舍成 "0"，定点 2 位有效数字
    const out = formatCost(0.00000045);
    expect(out).not.toBe("0");
    expect(Number(out)).toBeGreaterThan(0);
  });
  it("clamps decimal places to a max of 12 for extreme values", () => {
    const out = formatCost(1e-15);
    expect(out.length).toBeLessThanOrEqual("0.".length + 12);
  });
});

describe("formatCostUsd", () => {
  it("prefixes a $ sign", () => {
    expect(formatCostUsd(0)).toBe("$0");
    expect(formatCostUsd(1.5)).toBe("$1.50");
  });
});

describe("formatPercent", () => {
  it("defaults to 1 digit", () => {
    expect(formatPercent(98.7)).toBe("98.7%");
  });
  it("honours explicit digits", () => {
    expect(formatPercent(98.7, 0)).toBe("99%");
    expect(formatPercent(98.7, 2)).toBe("98.70%");
  });
});

describe("successRate", () => {
  it("returns 0 when total is 0 or negative", () => {
    expect(successRate(0, 0)).toBe(0);
    expect(successRate(5, -1)).toBe(0);
  });
  it("computes a percentage", () => {
    expect(successRate(99, 100)).toBe(99);
    expect(successRate(1, 4)).toBe(25);
  });
});

describe("sumTokens", () => {
  it("sums numeric parts, ignoring null/undefined/NaN", () => {
    expect(sumTokens(1, 2, 3)).toBe(6);
    expect(sumTokens(1, undefined, null, NaN, 4)).toBe(5);
    expect(sumTokens()).toBe(0);
  });
});

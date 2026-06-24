// @vitest-environment node
import { describe, it, expect } from "vitest";
import {
  levelColor,
  levelBg,
  successRateLevel,
  errorRateLevel,
  costLevel,
  successRateColor,
} from "./colorScale";

describe("levelColor / levelBg", () => {
  it("builds CSS var() references", () => {
    expect(levelColor("success")).toBe("var(--color-success)");
    expect(levelColor("danger")).toBe("var(--color-danger)");
    expect(levelBg("warning")).toBe("var(--color-warning-bg)");
    expect(levelBg("neutral")).toBe("var(--color-neutral-bg)");
  });
});

describe("successRateLevel", () => {
  it("neutral when no requests", () => {
    expect(successRateLevel(100, 0)).toBe("neutral");
    expect(successRateLevel(100, -1)).toBe("neutral");
  });
  it("thresholds 99 / 95", () => {
    expect(successRateLevel(99)).toBe("success");
    expect(successRateLevel(98.9)).toBe("warning");
    expect(successRateLevel(95)).toBe("warning");
    expect(successRateLevel(94.9)).toBe("danger");
  });
});

describe("errorRateLevel", () => {
  it("neutral when no requests", () => {
    expect(errorRateLevel(0, 0)).toBe("neutral");
  });
  it("thresholds 1 / 5", () => {
    expect(errorRateLevel(1)).toBe("success");
    expect(errorRateLevel(1.1)).toBe("warning");
    expect(errorRateLevel(5)).toBe("warning");
    expect(errorRateLevel(5.1)).toBe("danger");
  });
});

describe("costLevel", () => {
  it("neutral for non-positive cost", () => {
    expect(costLevel(0)).toBe("neutral");
    expect(costLevel(-3)).toBe("neutral");
  });
  it("uses default warnAt/dangerAt thresholds", () => {
    expect(costLevel(0.5)).toBe("success");
    expect(costLevel(5)).toBe("warning");
    expect(costLevel(50)).toBe("danger");
  });
  it("honours custom thresholds", () => {
    expect(costLevel(0.5, 0.1, 1)).toBe("warning");
    expect(costLevel(2, 0.1, 1)).toBe("danger");
  });
});

describe("successRateColor", () => {
  it("returns the level color string", () => {
    expect(successRateColor(99)).toBe("var(--color-success)");
    expect(successRateColor(100, 0)).toBe("var(--color-neutral)");
  });
});

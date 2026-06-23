// @vitest-environment node
import { describe, it, expect } from "vitest";
import {
  usageLevelToColor,
  cycleMsForTier,
  codingRemainPct,
  colorFromCodingRemainPct,
  codingTierLevel,
  balanceColorLevel,
} from "./usageColor";

const HOUR = 3_600_000;
const DAY = 24 * HOUR;

describe("usageLevelToColor", () => {
  it("maps backend level strings", () => {
    expect(usageLevelToColor("red")).toBe("danger");
    expect(usageLevelToColor("yellow")).toBe("warning");
    expect(usageLevelToColor("green")).toBe("success");
  });
  it("falls back to neutral for unknown/null/undefined", () => {
    expect(usageLevelToColor("neutral")).toBe("neutral");
    expect(usageLevelToColor(null)).toBe("neutral");
    expect(usageLevelToColor(undefined)).toBe("neutral");
    expect(usageLevelToColor("bogus")).toBe("neutral");
  });
});

describe("cycleMsForTier", () => {
  it("returns durations for known tiers", () => {
    expect(cycleMsForTier("five_hour")).toBe(5 * HOUR);
    expect(cycleMsForTier("weekly_limit")).toBe(7 * DAY);
    expect(cycleMsForTier("seven_day")).toBe(7 * DAY);
    expect(cycleMsForTier("mcp_monthly")).toBe(30 * DAY);
  });
  it("returns null for unknown tiers", () => {
    expect(cycleMsForTier("unknown")).toBeNull();
  });
});

describe("codingRemainPct", () => {
  it("returns 100 when utilization is 0 (saving)", () => {
    expect(codingRemainPct(0, 5 * HOUR, 5 * HOUR)).toBe(100);
  });
  it("returns 0 when no time elapsed (elapsedRatio <= 0)", () => {
    // remainMs == cycleMs → elapsedRatio = 0
    expect(codingRemainPct(50, 5 * HOUR, 5 * HOUR)).toBe(0);
  });
  it("returns 100 when pace below 1 (under budget)", () => {
    // util 10% used after 50% elapsed → pace 0.2 → remain clamps to 100
    expect(codingRemainPct(10, 2.5 * HOUR, 5 * HOUR)).toBe(100);
  });
  it("computes a mid remaining pct when pace > 1", () => {
    // util 80% after 40% elapsed → pace = 0.8/0.4 = 2 → 100/2 = 50
    expect(codingRemainPct(80, 3 * HOUR, 5 * HOUR)).toBe(50);
  });
});

describe("colorFromCodingRemainPct", () => {
  it("neutral for non-finite", () => {
    expect(colorFromCodingRemainPct(NaN)).toBe("neutral");
    expect(colorFromCodingRemainPct(Infinity)).toBe("neutral");
  });
  it("danger below 40", () => {
    expect(colorFromCodingRemainPct(39)).toBe("danger");
    expect(colorFromCodingRemainPct(0)).toBe("danger");
  });
  it("warning in [40, 60]", () => {
    expect(colorFromCodingRemainPct(40)).toBe("warning");
    expect(colorFromCodingRemainPct(60)).toBe("warning");
  });
  it("success above 60", () => {
    expect(colorFromCodingRemainPct(61)).toBe("success");
    expect(colorFromCodingRemainPct(100)).toBe("success");
  });
});

describe("codingTierLevel", () => {
  it("neutral on invalid utilization", () => {
    expect(codingTierLevel(NaN, DAY, DAY)).toBe("neutral");
    expect(codingTierLevel(-1, DAY, DAY)).toBe("neutral");
  });
  it("neutral when remain/cycle missing or non-positive cycle", () => {
    expect(codingTierLevel(50, null, DAY)).toBe("neutral");
    expect(codingTierLevel(50, DAY, null)).toBe("neutral");
    expect(codingTierLevel(50, DAY, 0)).toBe("neutral");
  });
  it("danger when quota exhausted (util >= 100)", () => {
    expect(codingTierLevel(100, 0, 5 * HOUR)).toBe("danger");
  });
  it("delegates to pace-based coloring otherwise", () => {
    // util 80% after 40% elapsed → remain 50 → warning
    expect(codingTierLevel(80, 3 * HOUR, 5 * HOUR)).toBe("warning");
    // saving → success
    expect(codingTierLevel(10, 2.5 * HOUR, 5 * HOUR)).toBe("success");
  });
});

describe("balanceColorLevel", () => {
  it("neutral on null/undefined/non-finite/negative", () => {
    expect(balanceColorLevel(null)).toBe("neutral");
    expect(balanceColorLevel(undefined)).toBe("neutral");
    expect(balanceColorLevel(NaN)).toBe("neutral");
    expect(balanceColorLevel(-1)).toBe("neutral");
  });
  it("danger below 1 day", () => {
    expect(balanceColorLevel(0)).toBe("danger");
    expect(balanceColorLevel(0.5)).toBe("danger");
  });
  it("warning below 3 days", () => {
    expect(balanceColorLevel(1)).toBe("warning");
    expect(balanceColorLevel(2.9)).toBe("warning");
  });
  it("success at or above 3 days", () => {
    expect(balanceColorLevel(3)).toBe("success");
    expect(balanceColorLevel(30)).toBe("success");
  });
});

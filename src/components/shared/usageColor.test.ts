// @vitest-environment node
import { describe, it, expect } from "vitest";
import {
  usageLevelToColor,
  cycleMsForTier,
  codingPaceDelta,
  colorFromPaceDelta,
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

describe("codingPaceDelta", () => {
  it("is 0 when usage tracks time exactly", () => {
    expect(codingPaceDelta(50, 2.5 * HOUR, 5 * HOUR)).toBe(0);
  });
  it("is positive when overspending", () => {
    // elapsed 50%, used 58% → +8pp
    expect(codingPaceDelta(58, 2.5 * HOUR, 5 * HOUR)).toBeCloseTo(8, 6);
  });
  it("is negative when under budget", () => {
    // 5h 周期剩 17m（elapsed 94.33%），已用 58% → -36.33pp
    expect(codingPaceDelta(58, 17 * 60_000, 5 * HOUR)).toBeCloseTo(-36.333333, 4);
  });
  it("clamps utilization and elapsed into 0-100", () => {
    expect(codingPaceDelta(150, 5 * HOUR, 5 * HOUR)).toBe(100);
    expect(codingPaceDelta(0, -HOUR, 5 * HOUR)).toBe(-100);
  });
});

describe("colorFromPaceDelta", () => {
  it("neutral for non-finite", () => {
    expect(colorFromPaceDelta(NaN)).toBe("neutral");
    expect(colorFromPaceDelta(Infinity)).toBe("neutral");
  });
  it("danger above +3", () => {
    expect(colorFromPaceDelta(3.1)).toBe("danger");
    expect(colorFromPaceDelta(50)).toBe("danger");
  });
  it("warning within ±3", () => {
    expect(colorFromPaceDelta(3)).toBe("warning");
    expect(colorFromPaceDelta(0)).toBe("warning");
    expect(colorFromPaceDelta(-3)).toBe("warning");
  });
  it("success below -3", () => {
    expect(colorFromPaceDelta(-3.1)).toBe("success");
    expect(colorFromPaceDelta(-100)).toBe("success");
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
  it("delegates to the pace-delta coloring otherwise", () => {
    // elapsed 40%, used 80% → +40pp → danger
    expect(codingTierLevel(80, 3 * HOUR, 5 * HOUR)).toBe("danger");
    // elapsed 50%, used 52% → +2pp → warning
    expect(codingTierLevel(52, 2.5 * HOUR, 5 * HOUR)).toBe("warning");
    // 用户实例：5h 剩 17m，配额剩 42%（已用 58%）→ -36.33pp → success
    expect(codingTierLevel(58, 17 * 60_000, 5 * HOUR)).toBe("success");
    // 周期刚开 1%，用了 2% → +1pp → warning（旧 pace 算法会判 danger）
    expect(codingTierLevel(2, 5 * HOUR * 0.99, 5 * HOUR)).toBe("warning");
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

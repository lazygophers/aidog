// @vitest-environment node
import { describe, it, expect, vi, afterEach } from "vitest";
import {
  formatNumber,
  formatBytes,
  formatCost,
  formatCostUsd,
  formatPercent,
  successRate,
  sumTokens,
  formatDateTime,
  formatRelativeTime,
  pad,
  clamp,
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

describe("formatBytes", () => {
  it("returns 0 B for zero / negative / NaN", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(-1)).toBe("0 B");
    expect(formatBytes(NaN)).toBe("0 B");
  });
  it("keeps bytes as integers", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1023)).toBe("1023 B");
  });
  it("steps up units at 1024 with 1 decimal", () => {
    expect(formatBytes(1024)).toBe("1.0 KB");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(1024 * 1024)).toBe("1.0 MB");
  });
  it("formats the real 7.4 GB log.db case", () => {
    // 7,401 MB —— 票 02 背景里用户实测的 log.db 体积
    expect(formatBytes(7401 * 1024 * 1024)).toBe("7.2 GB");
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

describe("formatDateTime", () => {
  it("returns null for empty / nullish / unparsable input", () => {
    expect(formatDateTime(null)).toBeNull();
    expect(formatDateTime(undefined)).toBeNull();
    expect(formatDateTime("")).toBeNull();
    expect(formatDateTime("not a date")).toBeNull();
    expect(formatDateTime(NaN)).toBeNull();
  });
  it("accepts both ISO strings and millisecond timestamps", () => {
    const ms = Date.UTC(2026, 5, 26, 12, 0, 0);
    expect(formatDateTime(ms)).toBe(new Date(ms).toLocaleString());
    expect(formatDateTime("2026-06-26T12:00:00Z")).toBe(new Date(ms).toLocaleString());
  });
});

describe("formatRelativeTime", () => {
  const NOW = Date.UTC(2026, 5, 26, 12, 0, 0);
  const ago = (ms: number) => formatRelativeTime(NOW - ms);
  const SEC = 1000, MIN = 60 * SEC, HR = 60 * MIN, DAY = 24 * HR;

  afterEach(() => vi.useRealTimers());

  it("returns null for empty / nullish / unparsable input", () => {
    expect(formatRelativeTime(null)).toBeNull();
    expect(formatRelativeTime(undefined)).toBeNull();
    expect(formatRelativeTime("")).toBeNull();
    expect(formatRelativeTime("garbage")).toBeNull();
  });

  it("picks the largest whole unit at each boundary", () => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
    expect(ago(0)).toBe("刚刚");
    expect(ago(59 * SEC)).toBe("刚刚");
    expect(ago(60 * SEC)).toBe("1 分钟前");
    expect(ago(59 * MIN)).toBe("59 分钟前");
    expect(ago(60 * MIN)).toBe("1 小时前");
    expect(ago(23 * HR)).toBe("23 小时前");
    expect(ago(24 * HR)).toBe("1 天前");
    expect(ago(29 * DAY)).toBe("29 天前");
    expect(ago(30 * DAY)).toBe("1 个月前");
    expect(ago(364 * DAY)).toBe("12 个月前");
    expect(ago(365 * DAY)).toBe("1 年前");
  });

  it("clamps future timestamps to 刚刚 instead of counting down", () => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
    expect(formatRelativeTime(NOW + DAY)).toBe("刚刚");
  });
});

describe("pad", () => {
  it("pads to 2 digits, leaves longer values intact", () => {
    expect(pad(7)).toBe("07");
    expect(pad(12)).toBe("12");
    expect(pad(0)).toBe("00");
    expect(pad(123)).toBe("123");
  });
});

describe("clamp", () => {
  it("clamps to both bounds and passes through in-range values", () => {
    expect(clamp(15, 1, 10)).toBe(10);
    expect(clamp(-5, 0, 100)).toBe(0);
    expect(clamp(50, 0, 100)).toBe(50);
  });
});

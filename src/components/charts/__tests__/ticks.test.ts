// @vitest-environment node
import { describe, it, expect } from "vitest";
import { niceTicks, formatTimeTick } from "../ticks";

describe("niceTicks", () => {
  it("0..100 → nice 1/2/5 步长刻度，含两端", () => {
    expect(niceTicks(0, 100, 5)).toEqual([0, 50, 100]);
  });

  it("3..97 with 6 ticks → step 20 covers to 100", () => {
    expect(niceTicks(3, 97, 6)).toEqual([0, 20, 40, 60, 80, 100]);
  });

  it("negative domain keeps sign and includes both ends", () => {
    expect(niceTicks(-5, 5, 5)).toEqual([-5, 0, 5]);
  });

  it("fractional domain snaps to clean decimal steps without fp noise", () => {
    const ticks = niceTicks(0.1, 0.4, 5);
    // rawStep=0.075 → step 0.1 → [0.1, 0.2, 0.3, 0.4]（floor(0.1/0.1)=1 起）
    expect(ticks).toEqual([0.1, 0.2, 0.3, 0.4]);
    for (const t of ticks) expect(Number.isInteger(t * 1e10)).toBe(true);
  });

  it("raw step exactly one magnitude → step 1 (norm<=1 branch)", () => {
    expect(niceTicks(0, 4, 5)).toEqual([0, 1, 2, 3, 4]);
  });

  it("步长整跨数据域 → 末刻度补一档覆盖 max（防按刻度定轴域后顶值被裁）", () => {
    // 0..7 with 4 ticks → step 5：循环止于 5，再补 10
    expect(niceTicks(0, 7, 4)).toEqual([0, 5, 10]);
    expect(niceTicks(0, 7, 4).every((t) => t >= 0)).toBe(true);
  });

  it("min === max → single tick", () => {
    expect(niceTicks(7, 7, 5)).toEqual([7]);
  });

  it("max < min → single tick (degenerate domain)", () => {
    expect(niceTicks(10, 2, 5)).toEqual([10]);
  });

  it("non-finite input → empty (axis falls back to recharts auto)", () => {
    expect(niceTicks(NaN, 5, 5)).toEqual([]);
    expect(niceTicks(0, Infinity, 5)).toEqual([]);
  });
});

describe("formatTimeTick", () => {
  // 本地时区构造，避免 UTC 偏移影响断言
  const t = new Date(2026, 5, 15, 9, 5, 0).getTime();
  const HOUR = 3_600_000;

  it("span ≤ 48h → HH:MM", () => {
    expect(formatTimeTick(t, 24 * HOUR)).toBe("09:05");
    expect(formatTimeTick(t, 48 * HOUR)).toBe("09:05");
  });

  it("span ≤ 60d → MM-DD", () => {
    expect(formatTimeTick(t, 7 * 24 * HOUR)).toBe("06-15");
    expect(formatTimeTick(t, 60 * 24 * HOUR)).toBe("06-15");
  });

  it("longer span → YYYY-MM", () => {
    expect(formatTimeTick(t, 200 * 24 * HOUR)).toBe("2026-06");
  });

  it("invalid timestamp → empty string", () => {
    expect(formatTimeTick(NaN, 24 * HOUR)).toBe("");
  });
});

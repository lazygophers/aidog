// @vitest-environment node
import { describe, it, expect } from "vitest";
import {
  shiftClock,
  normalizeWindow,
  isCurrentlyPeak,
  tzOffsetMinutes,
  utcToDisplay,
  displayToUtc,
  LOCAL_OFFSET_MINUTES,
} from "./peakHours";
import type { PeakWindow } from "../domains/platforms/defaults";

describe("shiftClock", () => {
  it("offset=0 恒等", () => {
    expect(shiftClock(14, 30, 0)).toEqual({ hour: 14, minute: 30 });
  });

  it("整时区往返（北京 +480 分钟）", () => {
    const displayed = shiftClock(6, 0, 480); // UTC 06:00 -> 北京 14:00
    expect(displayed).toEqual({ hour: 14, minute: 0 });
    const back = shiftClock(displayed.hour, displayed.minute, -480);
    expect(back).toEqual({ hour: 6, minute: 0 });
  });

  it("半时区往返（印度 +330 分钟）", () => {
    const displayed = shiftClock(8, 0, 330); // UTC 08:00 -> 印度 13:30
    expect(displayed).toEqual({ hour: 13, minute: 30 });
    const back = shiftClock(displayed.hour, displayed.minute, -330);
    expect(back).toEqual({ hour: 8, minute: 0 });
  });

  it("跨零点 mod 1440（正向进位）", () => {
    expect(shiftClock(23, 30, 60)).toEqual({ hour: 0, minute: 30 });
  });

  it("跨零点 mod 1440（负偏移借位）", () => {
    expect(shiftClock(0, 30, -60)).toEqual({ hour: 23, minute: 30 });
  });

  it("负偏移（美东 -300 分钟）", () => {
    expect(shiftClock(14, 0, -300)).toEqual({ hour: 9, minute: 0 });
  });
});

describe("normalizeWindow", () => {
  it("整数 hour 原样返回", () => {
    const w = { start_hour: 8, end_hour: 20, multiplier: 1.5 } as PeakWindow;
    expect(normalizeWindow(w)).toEqual(w);
  });

  it("8.0 视为整数不变（Number.isInteger(8.0) === true）", () => {
    const w = { start_hour: 8.0, end_hour: 20, multiplier: 1.5 } as PeakWindow;
    expect(normalizeWindow(w)).toEqual(w);
  });

  it("半时区脏数据拆分（8.5 -> 8:30）", () => {
    const w = { start_hour: 8.5, end_hour: 20, multiplier: 1.5 } as PeakWindow;
    const result = normalizeWindow(w);
    expect(result.start_hour).toBe(8);
    expect(result.start_minute).toBe(30);
    expect(result.end_hour).toBe(20);
    expect(result.end_minute).toBeUndefined();
  });

  it("已有 start_minute 时叠加进位", () => {
    const w = { start_hour: 8.5, start_minute: 40, end_hour: 20, multiplier: 1.5 } as PeakWindow;
    const result = normalizeWindow(w);
    // 8.5 -> floor=8, extra=30; 30+40=70 -> 借位 1 小时 10 分
    expect(result.start_hour).toBe(9);
    expect(result.start_minute).toBe(10);
  });

  it("both start/end 非整数各自独立归一", () => {
    const w = { start_hour: 8.5, end_hour: 20.25, multiplier: 1.5 } as PeakWindow;
    const result = normalizeWindow(w);
    expect(result.start_hour).toBe(8);
    expect(result.start_minute).toBe(30);
    expect(result.end_hour).toBe(20);
    expect(result.end_minute).toBe(15);
  });
});

describe("tzOffsetMinutes / utcToDisplay / displayToUtc", () => {
  it("utc 模式偏移恒为 0，local 模式取模块加载时的本地偏移", () => {
    expect(tzOffsetMinutes("utc")).toBe(0);
    expect(tzOffsetMinutes("local")).toBe(LOCAL_OFFSET_MINUTES);
  });

  it("utc 模式下换算是恒等的", () => {
    expect(utcToDisplay(6, 30, "utc")).toEqual({ hour: 6, minute: 30 });
    expect(displayToUtc(6, 30, "utc")).toEqual({ hour: 6, minute: 30 });
  });

  it("local 模式下 display→utc→display 往返回到原值", () => {
    const shown = utcToDisplay(6, 0, "local");
    expect(displayToUtc(shown.hour, shown.minute, "local")).toEqual({ hour: 6, minute: 0 });
  });
});

// hit() 的每个过滤维度独立测。基准时刻 2026-06-26T08:30:00Z = 周五(5)、26 号。
describe("isCurrentlyPeak — 时段 / 星期 / 日期 / model scope", () => {
  const NOW = Date.UTC(2026, 5, 26, 8, 30, 0);
  const win = (extra: Partial<PeakWindow>): PeakWindow =>
    ({ start_hour: 6, end_hour: 10, multiplier: 2, ...extra }) as PeakWindow;

  it("空 / null / undefined 窗口列表恒不命中", () => {
    expect(isCurrentlyPeak([], NOW)).toBe(false);
    expect(isCurrentlyPeak(null, NOW)).toBe(false);
    expect(isCurrentlyPeak(undefined, NOW)).toBe(false);
  });

  it("同天窗口是半开区间 [start, end)", () => {
    expect(isCurrentlyPeak([win({})], NOW)).toBe(true);
    expect(isCurrentlyPeak([win({ start_hour: 8, start_minute: 30 })], NOW)).toBe(true);
    expect(isCurrentlyPeak([win({ start_hour: 6, end_hour: 8, end_minute: 30 })], NOW)).toBe(false);
    expect(isCurrentlyPeak([win({ start_hour: 9, end_hour: 10 })], NOW)).toBe(false);
  });

  it("跨天窗口 end <= start 时是并集，start==end 退化为全天", () => {
    expect(isCurrentlyPeak([win({ start_hour: 22, end_hour: 9 })], NOW)).toBe(true);
    expect(isCurrentlyPeak([win({ start_hour: 22, end_hour: 6 })], NOW)).toBe(false);
    expect(isCurrentlyPeak([win({ start_hour: 5, end_hour: 5 })], NOW)).toBe(true);
  });

  it("越界 minute 被夹到 0..59 而非溢出成小时", () => {
    expect(isCurrentlyPeak([win({ start_hour: 8, start_minute: 99 })], NOW)).toBe(false);
    expect(isCurrentlyPeak([win({ start_hour: 8, start_minute: -10 })], NOW)).toBe(true);
  });

  it("days_of_week 用 0=Sun…6=Sat，缺省为每天", () => {
    expect(isCurrentlyPeak([win({ days_of_week: [5] })], NOW)).toBe(true);
    expect(isCurrentlyPeak([win({ days_of_week: [0, 6] })], NOW)).toBe(false);
  });

  it("days_of_month 与 days_of_week 取 AND", () => {
    expect(isCurrentlyPeak([win({ days_of_month: [26] })], NOW)).toBe(true);
    expect(isCurrentlyPeak([win({ days_of_month: [1] })], NOW)).toBe(false);
    expect(isCurrentlyPeak([win({ days_of_week: [5], days_of_month: [1] })], NOW)).toBe(false);
  });

  it("end_at 到期后窗口失效", () => {
    const sec = Math.floor(NOW / 1000);
    expect(isCurrentlyPeak([win({ end_at: sec + 1 })], NOW)).toBe(true);
    expect(isCurrentlyPeak([win({ end_at: sec })], NOW)).toBe(false);
  });

  it("model scope：空 requestModel 跳过过滤，通配取前缀", () => {
    const scoped = [win({ models: ["glm-5.2*", "kimi-k2"] })];
    expect(isCurrentlyPeak(scoped, NOW)).toBe(true); // 无 model 上下文
    expect(isCurrentlyPeak(scoped, NOW, "glm-5.2")).toBe(true);
    expect(isCurrentlyPeak(scoped, NOW, "glm-5.2-turbo")).toBe(true);
    expect(isCurrentlyPeak(scoped, NOW, "kimi-k2")).toBe(true);
    expect(isCurrentlyPeak(scoped, NOW, "kimi-k2-thinking")).toBe(false); // 非通配需精确
    expect(isCurrentlyPeak(scoped, NOW, "gpt-4")).toBe(false);
    expect(isCurrentlyPeak([win({ models: [] })], NOW, "gpt-4")).toBe(true); // 空列表=不限定
  });

  it("多窗口取任一命中", () => {
    expect(isCurrentlyPeak([win({ start_hour: 0, end_hour: 1 }), win({})], NOW)).toBe(true);
    expect(
      isCurrentlyPeak([win({ start_hour: 0, end_hour: 1 }), win({ start_hour: 20, end_hour: 22 })], NOW),
    ).toBe(false);
  });
});

describe("isCurrentlyPeak — start_at 生效期护栏", () => {
  // 全天窗口（start_hour=0/end_hour=24）令时段判定恒真，命中与否只受 start_at 门控。
  const w = { start_hour: 0, end_hour: 24, multiplier: 2.0, start_at: 1790784000 } as PeakWindow;

  it("nowMs 未越过 start_at → 不命中", () => {
    const beforeMs = (1790784000 - 1) * 1000;
    expect(isCurrentlyPeak([w], beforeMs)).toBe(false);
  });

  it("nowMs 越过 start_at → 命中", () => {
    const afterMs = (1790784000 + 1) * 1000;
    expect(isCurrentlyPeak([w], afterMs)).toBe(true);
  });
});

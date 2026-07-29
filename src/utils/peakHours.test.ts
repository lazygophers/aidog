// @vitest-environment node
import { describe, it, expect } from "vitest";
import { shiftClock, normalizeWindow } from "./peakHours";
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

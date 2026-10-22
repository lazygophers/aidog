// @vitest-environment node
import { describe, it, expect } from "vitest";
import {
  shiftClock,
  normalizeWindow,
  isCurrentlyPeak,
  currentPeakMultiplier,
  wallTimeInTz,
  tzOffsetMinutes,
  utcToDisplay,
  displayToUtc,
  LOCAL_OFFSET_MINUTES,
} from "./timeWindow";
import type { TimeWindow } from "../domains/platforms/defaults";
import goldenFixture from "../../src-tauri/crates/aidog_core/src/gateway/testdata/peak_golden.json";

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
    const w = { start_hour: 8, end_hour: 20, multiplier: 1.5 } as TimeWindow;
    expect(normalizeWindow(w)).toEqual(w);
  });

  it("8.0 视为整数不变（Number.isInteger(8.0) === true）", () => {
    const w = { start_hour: 8.0, end_hour: 20, multiplier: 1.5 } as TimeWindow;
    expect(normalizeWindow(w)).toEqual(w);
  });

  it("半时区脏数据拆分（8.5 -> 8:30）", () => {
    const w = { start_hour: 8.5, end_hour: 20, multiplier: 1.5 } as TimeWindow;
    const result = normalizeWindow(w);
    expect(result.start_hour).toBe(8);
    expect(result.start_minute).toBe(30);
    expect(result.end_hour).toBe(20);
    expect(result.end_minute).toBeUndefined();
  });

  it("已有 start_minute 时叠加进位", () => {
    const w = { start_hour: 8.5, start_minute: 40, end_hour: 20, multiplier: 1.5 } as TimeWindow;
    const result = normalizeWindow(w);
    // 8.5 -> floor=8, extra=30; 30+40=70 -> 借位 1 小时 10 分
    expect(result.start_hour).toBe(9);
    expect(result.start_minute).toBe(10);
  });

  it("both start/end 非整数各自独立归一", () => {
    const w = { start_hour: 8.5, end_hour: 20.25, multiplier: 1.5 } as TimeWindow;
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
// 镜像 Rust peak 时区测试（wall_time_* / resolve_multiplier_* / is_in_peak_window_*）。

// peak golden fixture：与 Rust include_str! 同一份共享真值（上面 import 的 peak_golden.json），
// 向量维护只改 JSON，两侧测试只做驱动。TS 特有 API 形状（null / undefined 入参）留在本文件。
type GoldenWallTimeCase = {
  name: string;
  ms: number;
  tz: string | null;
  hour: number;
  minute: number;
  weekday: number;
  day_of_month: number;
};
type GoldenResolveCase = {
  name: string;
  windows: TimeWindow[];
  ms: number;
  model: string;
  multiplier: number;
  in_peak: boolean;
};

describe("isCurrentlyPeak — 空 / null / undefined 窗口列表（TS API 形状）", () => {
  const NOW = Date.UTC(2026, 5, 26, 8, 30, 0);
  it("空数组 / null / undefined 恒不命中", () => {
    expect(isCurrentlyPeak([], NOW)).toBe(false);
    expect(isCurrentlyPeak(null, NOW)).toBe(false);
    expect(isCurrentlyPeak(undefined, NOW)).toBe(false);
  });

  it("models: [] 空列表 = 不限定（本地钉住：与 Rust Some([]) 现实现有分歧，统一语义另开票）", () => {
    const w = { start_hour: 6, end_hour: 10, multiplier: 2, models: [] } as TimeWindow;
    expect(isCurrentlyPeak([w], NOW, "gpt-4")).toBe(true);
  });
});

describe("peak golden fixture — wall_time（与 Rust 同一份真值）", () => {
  it.each(goldenFixture.wall_time_cases as unknown as GoldenWallTimeCase[])("$name", (c) => {
    expect(wallTimeInTz(c.ms, c.tz ?? undefined)).toEqual({
      hour: c.hour,
      minute: c.minute,
      weekday: c.weekday,
      dayOfMonth: c.day_of_month,
    });
  });
});

describe("peak golden fixture — resolve / is_peak（与 Rust 同一份真值）", () => {
  it.each(goldenFixture.resolve_cases as unknown as GoldenResolveCase[])("$name", (c) => {
    expect(isCurrentlyPeak(c.windows, c.ms, c.model)).toBe(c.in_peak);
  });
});

describe("currentPeakMultiplier — first-match 倍率（PeakSection pill 展示）", () => {
  const NOW = Date.UTC(2026, 5, 26, 8, 30, 0);
  it("无窗口 / 无命中 → 1.0", () => {
    expect(currentPeakMultiplier([], NOW)).toBe(1.0);
    expect(currentPeakMultiplier(undefined, NOW)).toBe(1.0);
    const miss = { start_hour: 20, end_hour: 22, multiplier: 3 } as TimeWindow;
    expect(currentPeakMultiplier([miss], NOW)).toBe(1.0);
  });

  it("命中返回 first-match 的 multiplier；非正数回落 1.0", () => {
    const first = { start_hour: 6, end_hour: 10, multiplier: 2 } as TimeWindow;
    const second = { start_hour: 8, end_hour: 12, multiplier: 5 } as TimeWindow;
    expect(currentPeakMultiplier([first, second], NOW)).toBe(2);
    const badMult = { start_hour: 6, end_hour: 10, multiplier: 0 } as TimeWindow;
    expect(currentPeakMultiplier([badMult], NOW)).toBe(1.0);
  });
});

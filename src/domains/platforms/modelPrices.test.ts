// modelPrices.ts 纯函数层：条目归一 + 标签拼接（grill-select-price spec 三轮拍板格式）。
import { describe, expect, it } from "vitest";
import {
  compactPriceLabel,
  fullPriceLines,
  toModelPriceInfo,
} from "./modelPrices";
import {
  effectiveUnitPrice,
  fmtPricePerUnit,
  type ModelPriceData,
} from "../../pages/ModelInfo/priceData";

// registry price 子树原单位 $/token；fullPriceLines 输出 $/M（perMillion 换算）。
const TIER = (v: number) => ({ input: v / 1_000_000, output: v * 5 / 1_000_000 });

describe("toModelPriceInfo", () => {
  it("空对象无任何可展示价 → null", () => {
    expect(toModelPriceInfo({})).toBeNull();
  });

  it("free=true 显式免费 → 保留标记", () => {
    const info = toModelPriceInfo({ free: true });
    expect(info?.free).toBe(true);
  });

  it("peak 子树展开为 peakInput/peakOutput", () => {
    const info = toModelPriceInfo({ input: 3e-6, peak: TIER(6) });
    expect(info?.peakInput).toBeCloseTo(6e-6);
    expect(info?.peakOutput).toBeCloseTo(30e-6);
  });

  it("非 token 计价条目 → unitLabel 现成串，无 token 四价", () => {
    const info = toModelPriceInfo({ unit: "image", unit_price: 0.004 });
    expect(info?.unitLabel).toBe("$0.00400 /image");
    expect(info?.input).toBeUndefined();
  });

  it("mp 计价条目 → unitLabel 展示 $/MP", () => {
    const info = toModelPriceInfo({ unit: "mp", unit_price: 0.03 });
    expect(info?.unitLabel).toBe("$0.030 /MP");
    expect(info?.input).toBeUndefined();
  });

  it("按次条目 time_tiers 生效后 unitLabel 切到档内价", () => {
    const startAt = 1794844800; // 2026-11-17 00:00 +08:00
    const info = toModelPriceInfo({
      unit: "second",
      unit_price: 0.000778,
      time_tiers: [{ start_at: startAt, unit_price: 0.00208333 }],
    });
    // 固定 nowMs 比较难（toModelPriceInfo 内部取当前时间）；用远未来/远过去两组断言
    const past = toModelPriceInfo({
      unit: "second",
      unit_price: 0.000778,
      time_tiers: [{ start_at: 1000, unit_price: 0.00208333 }],
    });
    expect(past?.unitLabel).toBe(fmt(0.00208333, "second"));
    expect(info?.unitLabel).toBe(fmt(0.000778, "second")); // 未生效 → 顶层价
  });

  it("按次条目 unit_tiers → 展示第一档（缺参兜底档）", () => {
    const info = toModelPriceInfo({
      unit: "image",
      unit_price: 0.05,
      unit_tiers: [
        { tier: "720p", unit_price: 0.1395 },
        { tier: "1080p", unit_price: 0.2479 },
      ],
    });
    expect(info?.unitLabel).toBe(fmt(0.1395, "image"));
  });

  it("effectiveUnitPrice 优先级：time 档 > unit_tiers 第一档 > 顶层", () => {
    const now = 1794844800 * 1000 + 1;
    const p: ModelPriceData = {
      unit: "second",
      unit_price: 0.1,
      unit_tiers: [{ tier: "720p", unit_price: 0.2 }],
      time_tiers: [{ start_at: 1794844800, unit_price: 0.3 }],
    };
    expect(effectiveUnitPrice(p, now)).toBe(0.3);
    expect(effectiveUnitPrice(p, 1794844800 * 1000 - 1)).toBe(0.2);
    expect(effectiveUnitPrice({ unit: "second" }, now)).toBeNull();
  });
});

/** 与展示层同一换算的期望值构造（fmtPricePerUnit 直接复用，断言不怕 formatMoney 精度）。 */
function fmt(v: number, unit: string): string {
  return fmtPricePerUnit(v, unit);
}

describe("compactPriceLabel", () => {
  it("in/out 双价", () => {
    expect(compactPriceLabel(toModelPriceInfo({ input: 3e-6, output: 15e-6 })!)).toBe("in $3.00 · out $15.00");
  });

  it("缺 output 只显 in", () => {
    expect(compactPriceLabel(toModelPriceInfo({ input: 3e-6 })!)).toBe("in $3.00");
  });

  it("只有缓存价无可显主价 → null", () => {
    expect(compactPriceLabel(toModelPriceInfo({ cache_read: 0.3e-6 })!)).toBeNull();
  });
});

describe("fullPriceLines", () => {
  it("四价两行，/M 标在最后一行行尾一次", () => {
    const lines = fullPriceLines(
      toModelPriceInfo({ input: 3e-6, output: 15e-6, cache_read: 0.3e-6, cache_write: 3.75e-6 })!,
      "峰",
    );
    expect(lines).toEqual(["in $3.00 · out $15.00", "cr $0.300 · cw $3.75 /M"]);
  });

  it("无缓存价单行，/M 直接缀行尾", () => {
    const lines = fullPriceLines(toModelPriceInfo({ input: 3e-6, output: 15e-6 })!, "峰");
    expect(lines).toEqual(["in $3.00 · out $15.00 /M"]);
  });

  it("带 peak 时第一行追加 峰 $in/$out", () => {
    const lines = fullPriceLines(
      toModelPriceInfo({ input: 3e-6, output: 15e-6, peak: TIER(6) })!,
      "峰",
    );
    expect(lines).toEqual(["in $3.00 · out $15.00 · 峰 $6.00/$30.00 /M"]);
  });

  it("只有缓存价 → 单行 cr（compact 为 null 但 full 仍可显）", () => {
    const d: ModelPriceData = { cache_read: 0.3e-6 };
    expect(fullPriceLines(toModelPriceInfo(d)!, "峰")).toEqual(["cr $0.300 /M"]);
  });
});

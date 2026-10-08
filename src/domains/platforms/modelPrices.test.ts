// modelPrices.ts 纯函数层：条目归一 + 标签拼接（grill-select-price spec 三轮拍板格式）。
import { describe, expect, it } from "vitest";
import {
  compactPriceLabel,
  fullPriceLines,
  toModelPriceInfo,
} from "./modelPrices";
import type { ModelPriceData } from "../../pages/ModelInfo/priceData";

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
});

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

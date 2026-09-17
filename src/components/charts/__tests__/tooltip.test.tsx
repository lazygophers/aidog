// tooltip 行 formatter 单测：行名映射（dataKey→label）与值格式化（含双轴按系列分派的 name 透传）
import { describe, it, expect } from "vitest";
import { renderToString } from "react-dom/server";
import { tooltipValueRows, TOOLTIP_THROTTLE_MS } from "../tooltip";

describe("TOOLTIP_THROTTLE_MS", () => {
  // 票 11 病灶 C 的回归闸：recharts 3 缺省 throttleDelay='raf'（每帧一次提交）。
  // 本值必须比一帧（60 Hz ≈ 16.7 ms）粗，否则节流等于没设。
  it("比一帧粗，否则节流无效", () => {
    expect(TOOLTIP_THROTTLE_MS).toBeGreaterThan(1000 / 60);
  });
});

describe("tooltipValueRows", () => {
  it("默认显示原始 name（dataKey）", () => {
    const html = renderToString(tooltipValueRows((n) => `$${n}`)(12.5, "s0", { color: "#000" }));
    expect(html).toContain("s0");
    expect(html).toContain("$12.5");
  });

  it("labelOf 映射 dataKey → config label（安全键不外露）", () => {
    const html = renderToString(
      tooltipValueRows(
        (n) => `${n}`,
        (name) => ({ s0: "深度求索", s1: "智谱" })[String(name)] ?? String(name),
      )(3, "s0", { color: "#000" }),
    );
    expect(html).toContain("深度求索");
    expect(html).not.toContain(">s0<");
  });

  it("fmt 第二参收到原始 dataKey（双轴调用方按 key 分派格式化）", () => {
    const rightKeys = new Set(["cost"]);
    const fmt = (n: number, name?: unknown) => (rightKeys.has(String(name)) ? `$${n}` : `${n}`);
    const html = renderToString(tooltipValueRows(fmt)(8, "cost", { color: "#000" }));
    expect(html).toContain("$8");
  });
});

import { describe, it, expect } from "vitest";
import { slotList, slotPrimary, slotsToForm, formToSlots, slotsToText, textToSlots } from "./slotValue";

describe("modelSlots", () => {
  it("slotList 兼容字符串/数组/空，trim 去空去重保序", () => {
    expect(slotList("a")).toEqual(["a"]);
    expect(slotList([" a ", "", "b", "a"])).toEqual(["a", "b"]);
    expect(slotList(undefined)).toEqual([]);
    expect(slotList("  ")).toEqual([]);
  });
  it("slotPrimary 取链首", () => {
    expect(slotPrimary(["x", "y"])).toBe("x");
    expect(slotPrimary(null)).toBe("");
  });
  it("slotsToForm / formToSlots 往返（空槽省略）", () => {
    const f = slotsToForm({ default: ["a", "b"], jev: "j" });
    expect(f.default).toBe("a");
    expect(f.sonnet).toBe("");
    expect(formToSlots(f)).toEqual({ default: ["a"], jev: ["j"] });
  });
  it("slotsToText / textToSlots 保留整条链（批量覆盖路径不降成单值）", () => {
    const t = slotsToText({ default: ["a", "b"], jev: "j" });
    expect(t.default).toBe("a, b");
    expect(t.jev).toBe("j");
    expect(textToSlots(t)).toEqual({ default: ["a", "b"], jev: ["j"] });
    expect(textToSlots({ default: " x ,, y,x，z" })).toEqual({ default: ["x", "y", "z"] });
  });
});

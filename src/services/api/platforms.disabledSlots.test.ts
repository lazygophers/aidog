import { describe, it, expect } from "vitest";
import { parseDisabledSlots, serializeDisabledSlots } from "./platforms";

describe("disabled_slots extra 读写", () => {
  it("解析：非数组 / 坏 JSON / 非字符串 → 空；去空去重", () => {
    expect(parseDisabledSlots("")).toEqual([]);
    expect(parseDisabledSlots('{"disabled_slots":"x"}')).toEqual([]);
    expect(parseDisabledSlots('{"disabled_slots":["a"," a ","",1,"b"]}')).toEqual(["a", "b"]);
  });
  it("旧键 soft_disabled_slots 视为关闭，与新键合并去重", () => {
    expect(parseDisabledSlots('{"soft_disabled_slots":["opus","default"]}')).toEqual(["opus", "default"]);
    expect(parseDisabledSlots('{"disabled_slots":["opus"],"soft_disabled_slots":["opus","gpt"]}')).toEqual(["opus", "gpt"]);
  });
  it("写回只写 disabled_slots 并删旧键，保留其余键；空数组删两键", () => {
    expect(JSON.parse(serializeDisabledSlots('{"x":1}', ["opus"]))).toEqual({ x: 1, disabled_slots: ["opus"] });
    expect(JSON.parse(serializeDisabledSlots('{"x":1,"soft_disabled_slots":["a"]}', ["a"]))).toEqual({ x: 1, disabled_slots: ["a"] });
    expect(JSON.parse(serializeDisabledSlots('{"x":1,"disabled_slots":["a"],"soft_disabled_slots":["b"]}', []))).toEqual({ x: 1 });
  });
});

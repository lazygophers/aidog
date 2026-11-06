import { describe, it, expect } from "vitest";
import { parseSoftDisabledSlots, serializeSoftDisabledSlots } from "./platforms";

describe("soft_disabled_slots extra 读写", () => {
  it("解析：非数组 / 坏 JSON / 非字符串 → 空；去空去重", () => {
    expect(parseSoftDisabledSlots("")).toEqual([]);
    expect(parseSoftDisabledSlots('{"soft_disabled_slots":"x"}')).toEqual([]);
    expect(parseSoftDisabledSlots('{"soft_disabled_slots":["a"," a ","",1,"b"]}')).toEqual(["a", "b"]);
  });
  it("写回保留其余键；空数组删键", () => {
    expect(JSON.parse(serializeSoftDisabledSlots('{"x":1}', ["opus"]))).toEqual({ x: 1, soft_disabled_slots: ["opus"] });
    expect(JSON.parse(serializeSoftDisabledSlots('{"x":1,"soft_disabled_slots":["a"]}', []))).toEqual({ x: 1 });
  });
});

// @vitest-environment node
import { describe, it, expect } from "vitest";
import { requestClassLabel, compactionLabel, parseToolDurations } from "./requestClass";

// t 桩：返回 `<key>|<fallback>`，既验 key 也验中文兜底。
const t = (key: string, fallback: string) => `${key}|${fallback}`;

describe("requestClass", () => {
  it("已知值走 logs.requestClass.<值> 并带中文兜底", () => {
    expect(requestClassLabel(t, "subagent")).toBe("logs.requestClass.subagent|子代理");
    expect(requestClassLabel(t, "side_reply")).toBe("logs.requestClass.side_reply|旁路回复");
  });
  it("未知取值原样展示；空串 -> '-'", () => {
    expect(requestClassLabel(t, "brand_new")).toBe("brand_new");
    expect(requestClassLabel(t, "")).toBe("-");
  });
  it("压缩原因：三个已知值走词表，未知原样，空串 '-'", () => {
    expect(compactionLabel(t, "auto")).toBe("logs.compaction.auto|自动");
    expect(compactionLabel(t, "manual")).toBe("logs.compaction.manual|手动");
    expect(compactionLabel(t, "reactive")).toBe("logs.compaction.reactive|被动");
    expect(compactionLabel(t, "other")).toBe("other");
    expect(compactionLabel(t, "")).toBe("-");
  });
  it("工具耗时解析：按 ; 与 = 拆，名字里含 = 取最后一个 =，坏段忽略", () => {
    expect(parseToolDurations("Bash=742;Read=9")).toEqual([{ name: "Bash", ms: 742 }, { name: "Read", ms: 9 }]);
    expect(parseToolDurations("a=b=5;;x;y=zz;")).toEqual([{ name: "a=b", ms: 5 }]);
    expect(parseToolDurations("")).toEqual([]);
  });
});

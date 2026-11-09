// @vitest-environment node
import { describe, it, expect } from "vitest";
import { REQUEST_CLASSES, REQUEST_CLASS_UNKNOWN, requestClassLabel } from "./requestClass";

// t 桩：返回 `<key>|<fallback>`，既验 key 也验中文兜底。
const t = (key: string, fallback: string) => `${key}|${fallback}`;

describe("requestClass", () => {
  it("六个已知取值按序", () => {
    expect(REQUEST_CLASSES).toEqual(["main", "subagent", "workflow", "compaction", "auxiliary", "side_reply"]);
  });
  it("已知值走 logs.requestClass.<值> 并带中文兜底", () => {
    expect(requestClassLabel(t, "subagent")).toBe("logs.requestClass.subagent|子代理");
    expect(requestClassLabel(t, "side_reply")).toBe("logs.requestClass.side_reply|旁路回复");
  });
  it("未知取值原样展示；空串 -> '-'", () => {
    expect(requestClassLabel(t, "brand_new")).toBe("brand_new");
    expect(requestClassLabel(t, "")).toBe("-");
  });
  it("未知过滤哨兵为 unknown", () => {
    expect(REQUEST_CLASS_UNKNOWN).toBe("unknown");
  });
});

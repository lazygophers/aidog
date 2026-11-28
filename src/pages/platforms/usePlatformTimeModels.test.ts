// usePlatformTimeModels（票 12）：6 个时段键并入 PlatformDraft 后，本 hook 只剩
// 「draft 键 + patch 适配 + windowsTz 展示态」。hydrate/serialize/reset 随之删除——
// 序列化语义由 platformDraft.draftToPayload 锁定（platformDraft.test.ts / 票 05 契约测试）。
import { describe, expect, it } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useReducer } from "react";
import { usePlatformTimeModels } from "./usePlatformTimeModels";
import { draftReducer, emptyDraft } from "../../domains/platforms/platformDraft";

/** 微型 harness：真实 draftReducer 持 draft，hook 经 dispatch patch。 */
function useHarness() {
  const [draft, dispatch] = useReducer(draftReducer, undefined, emptyDraft);
  const timemodels = usePlatformTimeModels(draft, dispatch);
  return { draft, timemodels };
}

describe("usePlatformTimeModels — draft + patch 适配（票 12）", () => {
  it("setPeak / setTimeModels patch 进同一份 draft", () => {
    const { result } = renderHook(() => useHarness());
    const peak = [{ start_hour: 9, end_hour: 10, multiplier: 2 }];
    const rules = [{ windows: [{ start_hour: 8, end_hour: 12, multiplier: 1 }], models: { default: "m" } }];
    act(() => {
      result.current.timemodels.setPeak(peak);
      result.current.timemodels.setTimeModels(rules);
    });
    expect(result.current.timemodels.peak).toEqual(peak);
    expect(result.current.timemodels.timeModels).toEqual(rules);
    expect(result.current.draft.peak).toEqual(peak);
    expect(result.current.draft.timeModels).toEqual(rules);
  });

  it("setter 支持函数式 updater（读当前 draft 键）", () => {
    const { result } = renderHook(() => useHarness());
    act(() => result.current.timemodels.setDirectModels(["m1"]));
    act(() => result.current.timemodels.setDirectModels(prev => [...prev, "m2"]));
    expect(result.current.timemodels.directModels).toEqual(["m1", "m2"]);
  });

  it("其余 3 键（disableDuringPeak / allowedClients / uaRules）同 patch 语义", () => {
    const { result } = renderHook(() => useHarness());
    act(() => {
      result.current.timemodels.setDisableDuringPeak(true);
      result.current.timemodels.setAllowedClients(["claude_code"]);
      result.current.timemodels.setUaRules([{ kind: "contains", pattern: "x" }]);
    });
    expect(result.current.timemodels.disableDuringPeak).toBe(true);
    expect(result.current.timemodels.allowedClients).toEqual(["claude_code"]);
    expect(result.current.timemodels.uaRules).toEqual([{ kind: "contains", pattern: "x" }]);
  });

  it("windowsTz 是本地展示态：切换不进 draft，patch 不碰它", () => {
    const { result } = renderHook(() => useHarness());
    expect(result.current.timemodels.windowsTz).toBe("local");
    act(() => result.current.timemodels.setWindowsTz("utc"));
    expect(result.current.timemodels.windowsTz).toBe("utc");
    expect(result.current.draft).not.toHaveProperty("windowsTz");
    act(() => result.current.timemodels.setPeak([{ start_hour: 1, end_hour: 2, multiplier: 3 }]));
    expect(result.current.timemodels.windowsTz).toBe("utc");
  });

  it("patch 只覆盖给出的键（reducer 纯 patch 语义穿透）", () => {
    const { result } = renderHook(() => useHarness());
    act(() => {
      result.current.timemodels.setAllowedClients(["codex"]);
      result.current.timemodels.setPeak([{ start_hour: 1, end_hour: 2, multiplier: 1 }]);
    });
    expect(result.current.timemodels.allowedClients).toEqual(["codex"]);
    expect(result.current.timemodels.peak).toHaveLength(1);
  });
});

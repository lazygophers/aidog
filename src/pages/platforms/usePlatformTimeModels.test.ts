import { describe, expect, it } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { usePlatformTimeModels } from "./usePlatformTimeModels";

const rule = { windows: [{ start_hour: 8, end_hour: 12, multiplier: 1 }], models: { default: "m" } };
const extra = JSON.stringify({
  keep: "yes",
  peak: [{ start_hour: 9, end_hour: 10, multiplier: 2 }],
  disable_during_peak: true,
  time_windows: [rule],
  direct_models: ["m1", "m2"],
});

describe("usePlatformTimeModels", () => {
  it("hydrates all four persisted fields with their actual values", () => {
    const { result } = renderHook(() => usePlatformTimeModels());
    act(() => result.current.hydrate(extra));
    expect(result.current.peak).toEqual([{ start_hour: 9, end_hour: 10, multiplier: 2 }]);
    expect(result.current.disableDuringPeak).toBe(true);
    expect(result.current.timeModels).toEqual([rule]);
    expect(result.current.directModels).toEqual(["m1", "m2"]);
  });

  it("serializes all four fields and preserves unrelated extra", () => {
    const { result } = renderHook(() => usePlatformTimeModels());
    act(() => {
      result.current.setPeak([{ start_hour: 1, end_hour: 2, multiplier: 3 }]);
      result.current.setDisableDuringPeak(true);
      result.current.setTimeModels([rule]);
      result.current.setDirectModels(["m1"]);
      result.current.setWindowsTz("utc");
    });
    const serialized = JSON.parse(result.current.serialize(JSON.stringify({ keep: "yes" })));
    expect(serialized.keep).toBe("yes");
    expect(serialized.peak).toEqual([{ start_hour: 1, end_hour: 2, multiplier: 3 }]);
    expect(serialized.disable_during_peak).toBe(true);
    expect(serialized.time_windows).toEqual([rule]);
    expect(serialized.direct_models).toEqual(["m1"]);
    expect(result.current.windowsTz).toBe("utc");
  });

  it("direct_models empty list is not written into extra", () => {
    const { result } = renderHook(() => usePlatformTimeModels());
    act(() => result.current.hydrate(extra));
    act(() => result.current.setDirectModels([]));
    const serialized = JSON.parse(result.current.serialize(extra));
    expect(serialized.direct_models).toBeUndefined();
    expect(serialized.keep).toBe("yes");
  });

  it("allowed_clients 回显 / 写回 / 清空删键", () => {
    const { result } = renderHook(() => usePlatformTimeModels());
    const e = JSON.stringify({ keep: "yes", allowed_clients: ["claude_code", "codex_cli"] });
    act(() => result.current.hydrate(e));
    expect(result.current.allowedClients).toEqual(["claude_code", "codex_cli"]);
    expect(JSON.parse(result.current.serialize(e)).allowed_clients).toEqual(["claude_code", "codex_cli"]);
    act(() => result.current.setAllowedClients([]));
    expect(JSON.parse(result.current.serialize(e))).toEqual({ keep: "yes" });
  });

  it("deletes all four persisted keys when cleared", () => {
    const { result } = renderHook(() => usePlatformTimeModels());
    act(() => result.current.hydrate(extra));
    act(() => result.current.reset());
    const serialized = JSON.parse(result.current.serialize(extra));
    expect(serialized).toEqual({ keep: "yes" });
    expect(result.current.windowsTz).toBe("local");
    expect(result.current.directModels).toEqual([]);
  });
});

describe("usePlatformTimeModels — client_ua_rules", () => {
  it("hydrate 回显 / serialize 写回 / 清空删键，其余键保留", () => {
    const rules = [{ kind: "contains", pattern: "openviking" }];
    const ex = JSON.stringify({ keep: "yes", client_ua_rules: rules });
    const { result } = renderHook(() => usePlatformTimeModels());
    act(() => result.current.hydrate(ex));
    expect(result.current.uaRules).toEqual(rules);
    expect(JSON.parse(result.current.serialize(ex)).client_ua_rules).toEqual(rules);
    act(() => result.current.setUaRules([]));
    const cleared = JSON.parse(result.current.serialize(ex));
    expect(cleared.client_ua_rules).toBeUndefined();
    expect(cleared.keep).toBe("yes");
    act(() => result.current.reset());
    expect(result.current.uaRules).toEqual([]);
  });
});

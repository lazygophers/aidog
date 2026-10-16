import { describe, expect, it } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { usePlatformTimeModels } from "./usePlatformTimeModels";

const rule = { windows: [{ start_hour: 8, end_hour: 12, multiplier: 1 }], models: { default: "m" } };
const extra = JSON.stringify({
  keep: "yes",
  peak: [{ start_hour: 9, end_hour: 10, multiplier: 2 }],
  disable_during_peak: true,
  time_windows: [rule],
});

describe("usePlatformTimeModels", () => {
  it("hydrates all three persisted fields with their actual values", () => {
    const { result } = renderHook(() => usePlatformTimeModels());
    act(() => result.current.hydrate(extra));
    expect(result.current.peak).toEqual([{ start_hour: 9, end_hour: 10, multiplier: 2 }]);
    expect(result.current.disableDuringPeak).toBe(true);
    expect(result.current.timeModels).toEqual([rule]);
  });

  it("serializes all three fields and preserves unrelated extra", () => {
    const { result } = renderHook(() => usePlatformTimeModels());
    act(() => {
      result.current.setPeak([{ start_hour: 1, end_hour: 2, multiplier: 3 }]);
      result.current.setDisableDuringPeak(true);
      result.current.setTimeModels([rule]);
      result.current.setWindowsTz("utc");
    });
    const serialized = JSON.parse(result.current.serialize(JSON.stringify({ keep: "yes" })));
    expect(serialized.keep).toBe("yes");
    expect(serialized.peak).toEqual([{ start_hour: 1, end_hour: 2, multiplier: 3 }]);
    expect(serialized.disable_during_peak).toBe(true);
    expect(serialized.time_windows).toEqual([rule]);
    expect(result.current.windowsTz).toBe("utc");
  });

  it("deletes all three persisted keys when cleared", () => {
    const { result } = renderHook(() => usePlatformTimeModels());
    act(() => result.current.hydrate(extra));
    act(() => result.current.reset());
    const serialized = JSON.parse(result.current.serialize(extra));
    expect(serialized).toEqual({ keep: "yes" });
    expect(result.current.windowsTz).toBe("local");
  });
});

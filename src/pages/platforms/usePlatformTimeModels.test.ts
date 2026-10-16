import { describe, expect, it } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { usePlatformTimeModels } from "./usePlatformTimeModels";

const extra = JSON.stringify({
  peak: [{ start_hour: 9, end_hour: 10, multiplier: 2 }],
  disable_during_peak: true,
  time_windows: [{ windows: [{ start_hour: 8, end_hour: 12, multiplier: 1 }], models: { default: "m" } }],
});

describe("usePlatformTimeModels", () => {
  it("hydrates, serializes, and resets timemodel fields without changing unrelated extra", () => {
    const { result } = renderHook(() => usePlatformTimeModels());
    act(() => result.current.hydrate(extra));
    expect(result.current.peak).toHaveLength(1);
    expect(result.current.disableDuringPeak).toBe(true);
    expect(result.current.timeModels).toHaveLength(1);

    act(() => result.current.setWindowsTz("utc"));
    expect(result.current.serialize(JSON.stringify({ keep: "yes" }))).toContain('"keep":"yes"');

    act(() => result.current.reset());
    expect(result.current.peak).toEqual([]);
    expect(result.current.windowsTz).toBe("local");
    expect(result.current.disableDuringPeak).toBe(false);
    expect(result.current.timeModels).toEqual([]);
  });
});

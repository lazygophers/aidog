import { describe, it, expect, vi, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useCountUp } from "./useCountUp";

const setReduced = (v: boolean) => vi.stubGlobal("matchMedia", () => ({ matches: v }));

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("useCountUp", () => {
  it("reduced-motion：直接返回目标值", () => {
    setReduced(true);
    const { result, rerender } = renderHook(({ t }) => useCountUp(t), { initialProps: { t: 10 } });
    expect(result.current).toBe(10);
    rerender({ t: 50 });
    expect(result.current).toBe(50);
  });

  it("非有限值直接返回", () => {
    setReduced(false);
    const { result, rerender } = renderHook(({ t }) => useCountUp(t), { initialProps: { t: 1 } });
    rerender({ t: NaN });
    expect(result.current).toBeNaN();
  });

  it("目标不变不起动画", () => {
    setReduced(false);
    const raf = vi.spyOn(window, "requestAnimationFrame");
    const { rerender } = renderHook(({ t }) => useCountUp(t), { initialProps: { t: 5 } });
    rerender({ t: 5 });
    expect(raf).not.toHaveBeenCalled();
  });

  it("动画逐帧逼近并落在目标值", () => {
    setReduced(false);
    const cbs: FrameRequestCallback[] = [];
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((cb) => {
      cbs.push(cb);
      return cbs.length;
    });
    const cancel = vi.spyOn(window, "cancelAnimationFrame").mockImplementation(() => {});
    vi.spyOn(performance, "now").mockReturnValue(1000);
    const { result, rerender, unmount } = renderHook(({ t }) => useCountUp(t, 600), { initialProps: { t: 0 } });
    rerender({ t: 100 });
    act(() => cbs.shift()!(1300)); // p=0.5, eased=0.875
    expect(result.current).toBeCloseTo(87.5);
    act(() => cbs.shift()!(2000)); // p=1
    expect(result.current).toBe(100);
    unmount();
    expect(cancel).toHaveBeenCalled();
  });
});

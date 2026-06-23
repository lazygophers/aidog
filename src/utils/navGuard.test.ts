// @vitest-environment node
import { describe, it, expect, vi, beforeEach } from "vitest";
import { registerNavGuard, requestNavigation } from "./navGuard";

// navGuard 持模块级单例 activeGuard，用例间须 unregister 复位。
let cleanups: Array<() => void> = [];
beforeEach(() => {
  // 清理上一个用例残留的 guard
  for (const c of cleanups) c();
  cleanups = [];
});

describe("requestNavigation", () => {
  it("proceeds immediately when no guard registered", () => {
    const proceed = vi.fn();
    requestNavigation(proceed);
    expect(proceed).toHaveBeenCalledTimes(1);
  });

  it("delegates to the active guard which decides", () => {
    const guard = vi.fn((proceed: () => void) => {
      // 模拟用户确认后才放行
      proceed();
    });
    cleanups.push(registerNavGuard(guard));
    const proceed = vi.fn();
    requestNavigation(proceed);
    expect(guard).toHaveBeenCalledTimes(1);
    expect(proceed).toHaveBeenCalledTimes(1);
  });

  it("guard may withhold proceed (cancel)", () => {
    const guard = vi.fn((_proceed: () => void) => {
      // 用户取消，不调用 proceed
    });
    cleanups.push(registerNavGuard(guard));
    const proceed = vi.fn();
    requestNavigation(proceed);
    expect(guard).toHaveBeenCalledTimes(1);
    expect(proceed).not.toHaveBeenCalled();
  });
});

describe("registerNavGuard", () => {
  it("last registration wins", () => {
    const first = vi.fn();
    const second = vi.fn();
    cleanups.push(registerNavGuard(first));
    cleanups.push(registerNavGuard(second));
    requestNavigation(() => {});
    expect(second).toHaveBeenCalledTimes(1);
    expect(first).not.toHaveBeenCalled();
  });

  it("unregister only clears the guard if still active", () => {
    const first = vi.fn();
    const second = vi.fn();
    const unregFirst = registerNavGuard(first);
    cleanups.push(registerNavGuard(second)); // second now active
    // unregistering first must NOT clear second
    unregFirst();
    requestNavigation(() => {});
    expect(second).toHaveBeenCalledTimes(1);
  });

  it("unregister of the active guard restores no-guard behaviour", () => {
    const guard = vi.fn();
    const unreg = registerNavGuard(guard);
    unreg();
    const proceed = vi.fn();
    requestNavigation(proceed);
    expect(guard).not.toHaveBeenCalled();
    expect(proceed).toHaveBeenCalledTimes(1);
  });
});

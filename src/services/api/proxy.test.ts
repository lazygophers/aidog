import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

const hoisted = vi.hoisted(() => ({
  handler: null as null | ((e: { payload: unknown }) => void),
  listen: vi.fn(),
  invoke: vi.fn(),
}));
vi.mock("../transport", () => ({ invoke: hoisted.invoke, listen: hoisted.listen }));

import { onProxyLogUpdated, PROXY_LOG_UPDATED, isProxyStartError } from "./proxy";

beforeEach(() => {
  vi.useFakeTimers();
  hoisted.listen.mockImplementation((_e: string, cb: (e: { payload: unknown }) => void) => {
    hoisted.handler = cb;
    return Promise.resolve(() => {});
  });
});
afterEach(() => vi.useRealTimers());

describe("onProxyLogUpdated", () => {
  it("单底层 listen 扇出，debounce 合并突发，cleanup 注销", () => {
    const a = vi.fn();
    const b = vi.fn();
    const offA = onProxyLogUpdated(a, 100);
    const offB = onProxyLogUpdated(b, 100);
    expect(hoisted.listen).toHaveBeenCalledTimes(1);
    expect(hoisted.listen.mock.calls[0][0]).toBe(PROXY_LOG_UPDATED);
    hoisted.handler!({ payload: 7 });
    hoisted.handler!({ payload: "not-a-number" });
    vi.advanceTimersByTime(100);
    expect(a).toHaveBeenCalledTimes(1);
    expect(b).toHaveBeenCalledTimes(1);
    offA();
    hoisted.handler!({ payload: 1 });
    vi.advanceTimersByTime(100);
    expect(a).toHaveBeenCalledTimes(1);
    expect(b).toHaveBeenCalledTimes(2);
    offB();
  });

  it("未触发的 timer 被 cleanup 清掉", () => {
    const good = vi.fn();
    const cancelled = vi.fn();
    const offs = [onProxyLogUpdated(vi.fn(), 10), onProxyLogUpdated(good, 10)];
    const off3 = onProxyLogUpdated(cancelled, 10);
    hoisted.handler!({ payload: null });
    off3();
    vi.advanceTimersByTime(10);
    expect(good).toHaveBeenCalled();
    expect(cancelled).not.toHaveBeenCalled();
    offs.forEach((o) => o());
  });
});

describe("isProxyStartError", () => {
  it("需同时有 kind 与 port", () => {
    expect(isProxyStartError({ kind: "in_use", port: 1 })).toBe(true);
    expect(isProxyStartError({ kind: "x" })).toBe(false);
    expect(isProxyStartError("s")).toBe(false);
    expect(isProxyStartError(null)).toBe(false);
  });
});

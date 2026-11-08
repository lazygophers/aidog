import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

const hoisted = vi.hoisted(() => ({
  handler: null as null | ((e: { payload: unknown }) => void),
  listen: vi.fn(),
  invoke: vi.fn(),
}));
vi.mock("../transport", () => ({ invoke: hoisted.invoke, listen: hoisted.listen }));

import { onProxyLogUpdated, PROXY_LOG_UPDATED, isProxyStartError } from "./proxy";

function setVisible(v: boolean) {
  Object.defineProperty(document, "visibilityState", { value: v ? "visible" : "hidden", configurable: true });
  Object.defineProperty(document, "hidden", { value: !v, configurable: true });
}

beforeEach(() => {
  vi.useFakeTimers();
  setVisible(true);
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

  it("隐藏时不触发回调；恢复可见时补拉一次（覆盖隐藏期积累）", () => {
    const cb = vi.fn();
    const off = onProxyLogUpdated(cb, 100);
    setVisible(false);
    hoisted.handler!({ payload: 1 });
    hoisted.handler!({ payload: 2 });
    hoisted.handler!({ payload: 3 });
    vi.advanceTimersByTime(200);
    expect(cb).not.toHaveBeenCalled();
    setVisible(true);
    document.dispatchEvent(new Event("visibilitychange"));
    expect(cb).toHaveBeenCalledTimes(1);
    off();
    // 清掉 afterEach 外残留的 visibilitychange 监听（off 已注销）
    setVisible(true);
  });

  it("可见期正常 debounce；恢复可见但无隐藏期积累不触发", () => {
    const cb = vi.fn();
    const off = onProxyLogUpdated(cb, 100);
    hoisted.handler!({ payload: 1 });
    vi.advanceTimersByTime(99);
    expect(cb).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(cb).toHaveBeenCalledTimes(1);
    // 无 pending 时 visibilitychange 不应补拉
    document.dispatchEvent(new Event("visibilitychange"));
    expect(cb).toHaveBeenCalledTimes(1);
    off();
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

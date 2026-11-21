import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

const hoisted = vi.hoisted(() => ({ handler: null as null | (() => void), listen: vi.fn(), invoke: vi.fn() }));
vi.mock("../transport", () => ({ invoke: hoisted.invoke, listen: hoisted.listen }));

import { onRegistryUpdated, REGISTRY_UPDATED } from "./pricing";

function setVisible(v: boolean) {
  Object.defineProperty(document, "visibilityState", { value: v ? "visible" : "hidden", configurable: true });
  Object.defineProperty(document, "hidden", { value: !v, configurable: true });
}

beforeEach(() => {
  vi.useFakeTimers();
  setVisible(true);
  hoisted.listen.mockImplementation((_e: string, cb: () => void) => {
    hoisted.handler = cb;
    return Promise.resolve(() => {});
  });
});
afterEach(() => vi.useRealTimers());

describe("onRegistryUpdated", () => {
  it("单底层 listen 扇出，debounce 合并突发", () => {
    const a = vi.fn();
    const b = vi.fn();
    const offA = onRegistryUpdated(a, 100);
    const offB = onRegistryUpdated(b, 100);
    expect(hoisted.listen).toHaveBeenCalledTimes(1);
    expect(hoisted.listen.mock.calls[0][0]).toBe(REGISTRY_UPDATED);
    hoisted.handler!();
    hoisted.handler!();
    vi.advanceTimersByTime(100);
    expect(a).toHaveBeenCalledTimes(1);
    expect(b).toHaveBeenCalledTimes(1);
    offA();
    hoisted.handler!();
    vi.advanceTimersByTime(100);
    expect(a).toHaveBeenCalledTimes(1);
    expect(b).toHaveBeenCalledTimes(2);
    offB();
  });

  it("隐藏时不触发回调；恢复可见时补拉一次（覆盖隐藏期积累）", () => {
    const cb = vi.fn();
    const off = onRegistryUpdated(cb, 100);
    setVisible(false);
    hoisted.handler!();
    hoisted.handler!();
    vi.advanceTimersByTime(200);
    expect(cb).not.toHaveBeenCalled();
    setVisible(true);
    document.dispatchEvent(new Event("visibilitychange"));
    expect(cb).toHaveBeenCalledTimes(1);
    off();
    setVisible(true);
  });

  it("可见期正常 debounce；无隐藏期积累时 visibilitychange 不补拉", () => {
    const cb = vi.fn();
    const off = onRegistryUpdated(cb, 100);
    hoisted.handler!();
    vi.advanceTimersByTime(99);
    expect(cb).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(cb).toHaveBeenCalledTimes(1);
    document.dispatchEvent(new Event("visibilitychange"));
    expect(cb).toHaveBeenCalledTimes(1);
    off();
  });

  it("cleanup 清掉未触发的 timer", () => {
    const good = vi.fn();
    const cancelled = vi.fn();
    const off1 = onRegistryUpdated(vi.fn(), 10);
    const off2 = onRegistryUpdated(good, 10);
    const off3 = onRegistryUpdated(cancelled, 10);
    hoisted.handler!();
    off3();
    vi.advanceTimersByTime(10);
    expect(good).toHaveBeenCalled();
    expect(cancelled).not.toHaveBeenCalled();
    off1();
    off2();
  });
});

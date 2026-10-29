import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

const hoisted = vi.hoisted(() => ({ handler: null as null | (() => void), listen: vi.fn(), invoke: vi.fn() }));
vi.mock("../transport", () => ({ invoke: hoisted.invoke, listen: hoisted.listen }));

import { onRegistryUpdated, REGISTRY_UPDATED } from "./pricing";

beforeEach(() => {
  vi.useFakeTimers();
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

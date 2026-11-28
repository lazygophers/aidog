// useBatchAction 纯 hook 测试（票 08）：四套批量操作共用的 target/busy/toast/刷新样板。
import { describe, it, expect, vi, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useBatchAction } from "./useBatchAction";

function setup() {
  const deps = { silentReload: vi.fn(), onGroupsChanged: vi.fn(), onToast: vi.fn() };
  const h = renderHook(() => useBatchAction<{ id: number }>(deps));
  return { deps, result: h.result };
}

afterEach(() => vi.useRealTimers());

describe("useBatchAction", () => {
  it("初值：target null / busy false", () => {
    const { result } = setup();
    expect(result.current.target).toBeNull();
    expect(result.current.busy).toBe(false);
  });

  it("成功：target 清空 / busy 复位 / 刷新 / onSuccess / 成功 toast + 3s 清除", async () => {
    vi.useFakeTimers();
    const { deps, result } = setup();
    act(() => result.current.setTarget({ id: 1 }));
    const onSuccess = vi.fn();
    await act(async () => {
      await result.current.runBatch({
        api: async () => ({ applied: 2 }),
        doneText: (n) => `ok-${n}`,
        failPrefix: "fail",
        onSuccess,
      });
    });
    expect(result.current.target).toBeNull();
    expect(result.current.busy).toBe(false);
    expect(deps.silentReload).toHaveBeenCalledTimes(1);
    expect(deps.onGroupsChanged).toHaveBeenCalledTimes(1);
    expect(onSuccess).toHaveBeenCalledTimes(1);
    expect(deps.onToast).toHaveBeenLastCalledWith({ text: "ok-2", ok: true });
    act(() => vi.advanceTimersByTime(3000));
    expect(deps.onToast).toHaveBeenLastCalledWith(null);
  });

  it("失败：target 清空 / busy 复位 / 失败 toast / 不刷新 / onSuccess 不触发", async () => {
    const { deps, result } = setup();
    act(() => result.current.setTarget({ id: 1 }));
    const onSuccess = vi.fn();
    await act(async () => {
      await result.current.runBatch({
        api: async () => { throw new Error("boom"); },
        doneText: (n) => `ok-${n}`,
        failPrefix: "fail",
        onSuccess,
      });
    });
    expect(result.current.target).toBeNull();
    expect(result.current.busy).toBe(false);
    expect(deps.silentReload).not.toHaveBeenCalled();
    expect(deps.onGroupsChanged).not.toHaveBeenCalled();
    expect(onSuccess).not.toHaveBeenCalled();
    expect(deps.onToast).toHaveBeenLastCalledWith({ text: expect.stringContaining("fail"), ok: false });
  });
});

import { describe, it, expect } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useCliProxySelection } from "./useCliProxySelection";

describe("useCliProxySelection", () => {
  it("初始未进入选择态，selectedCount 为 0", () => {
    const r = renderHook(() => useCliProxySelection([1, 2, 3]));
    expect(r.result.current.selectMode).toBe(false);
    expect(r.result.current.selectedCount).toBe(0);
  });

  it("enter 进入选择态，exit 清空选择并关闭所有 batch modal", () => {
    const r = renderHook(() => useCliProxySelection([1, 2, 3]));
    act(() => r.result.current.enter());
    expect(r.result.current.selectMode).toBe(true);
    act(() => r.result.current.toggle(1));
    act(() => r.result.current.openBatchDelete());
    expect(r.result.current.selectedCount).toBe(1);
    expect(r.result.current.batchDeleteOpen).toBe(true);

    act(() => r.result.current.exit());
    expect(r.result.current.selectMode).toBe(false);
    expect(r.result.current.selectedCount).toBe(0);
    expect(r.result.current.batchDeleteOpen).toBe(false);
  });

  it("toggle 是幂等开关：同一 id 两次 toggle 回到未选中", () => {
    const r = renderHook(() => useCliProxySelection([1, 2, 3]));
    act(() => r.result.current.toggle(1));
    expect(r.result.current.isSelected(1)).toBe(true);
    act(() => r.result.current.toggle(1));
    expect(r.result.current.isSelected(1)).toBe(false);
  });

  it("toggleAll 全选，isAllSelected 为 true；再 toggleAll 清空", () => {
    const r = renderHook(() => useCliProxySelection([1, 2, 3]));
    act(() => r.result.current.toggleAll());
    expect(r.result.current.selectedCount).toBe(3);
    expect(r.result.current.isAllSelected).toBe(true);

    act(() => r.result.current.toggleAll());
    expect(r.result.current.selectedCount).toBe(0);
    expect(r.result.current.isAllSelected).toBe(false);
  });

  it("空 id 列表时 isAllSelected 恒为 false（避免 0===0 误判全选）", () => {
    const r = renderHook(() => useCliProxySelection([]));
    expect(r.result.current.isAllSelected).toBe(false);
  });

  it("openBatchModels 重置 batchModelsText 为空串", () => {
    const r = renderHook(() => useCliProxySelection([1]));
    act(() => r.result.current.setBatchModelsText("gpt-4"));
    act(() => r.result.current.closeBatchModels());
    act(() => r.result.current.openBatchModels());
    expect(r.result.current.batchModelsText).toBe("");
  });

  it("openBatchQuota 重置 batchQuotaType 为 none", () => {
    const r = renderHook(() => useCliProxySelection([1]));
    act(() => r.result.current.setBatchQuotaType("newapi"));
    act(() => r.result.current.closeBatchQuota());
    act(() => r.result.current.openBatchQuota());
    expect(r.result.current.batchQuotaType).toBe("none");
  });
});

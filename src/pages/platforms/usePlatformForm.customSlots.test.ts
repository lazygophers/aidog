// usePlatformForm 的自定义槽位：保存 payload 写 extra.custom_slots（孤儿键丢弃、空槽省略），定义加载失败只记日志。
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import { useRef } from "react";
import { platformApi } from "../../services/api";
import { customSlotsApi } from "../../services/api/customSlots";
import { usePlatformForm } from "./usePlatformForm";

function useForm() {
  const platformsEpochRef = useRef(0);
  const groupsReloadRef = useRef<(() => void) | null>(null);
  const consumedEditPidRef = useRef<number | null>(null);
  const deps = {
    t: ((k: string, f?: string) => f ?? k) as any,
    platforms: [],
    setPlatforms: vi.fn(),
    platformsEpochRef,
    quota: {} as any,
    setGroupDetails: vi.fn(),
    handleGroupsChanged: vi.fn(),
    groupsReloadRef,
    setToast: vi.fn(),
    breakerDefaults: {} as any,
    setUsageMap: vi.fn(),
    setLastTestMap: vi.fn(),
    onNavigate: vi.fn(),
    consumedEditPidRef,
  };
  return usePlatformForm(deps as any);
}

describe("custom slots in save payload", () => {
  beforeEach(() => {
    vi.spyOn(platformApi, "update").mockResolvedValue(undefined as any);
    vi.spyOn(console, "error").mockImplementation(() => {});
  });
  afterEach(() => vi.restoreAllMocks());

  const prepare = (result: { current: ReturnType<typeof useForm> }) => {
    result.current.setEditing({ id: 7, platform_type: "openai" } as any);
    result.current.setProtocol("openai");
    result.current.setName("P");
    result.current.setEndpoints([{ protocol: "openai", base_url: "https://example.test/v1" }] as any);
  };

  it("writes defined slots, drops orphan keys, omits empty chains", async () => {
    vi.spyOn(customSlotsApi, "get").mockResolvedValue([
      { name: "fast", match: "contains" },
      { name: "bg", match: "exact" },
    ]);
    const { result } = renderHook(() => useForm());
    act(() => { prepare(result); result.current.setShowForm(true); });
    await waitFor(() => expect(result.current.customSlotDefs).not.toBeNull());
    act(() => result.current.setCustomSlotValues({ fast: ["f1", "f2"], bg: [], gone: ["x"] }));
    await act(() => result.current.handleSave());
    const arg = (platformApi.update as any).mock.calls[0][0];
    expect(JSON.parse(arg.extra).custom_slots).toEqual({ fast: ["f1", "f2"] });
  });

  it("definitions load failure is logged, not swallowed, and does not touch extra.custom_slots", async () => {
    vi.spyOn(customSlotsApi, "get").mockRejectedValue("boom");
    const { result } = renderHook(() => useForm());
    act(() => { prepare(result); result.current.setExtra('{"custom_slots":{"fast":["keep"]}}'); result.current.setShowForm(true); });
    await waitFor(() => expect(console.error).toHaveBeenCalledWith(expect.stringContaining("custom slots"), "boom"));
    expect(result.current.customSlotDefs).toBeNull();
    await act(() => result.current.handleSave());
    const arg = (platformApi.update as any).mock.calls[0][0];
    expect(JSON.parse(arg.extra).custom_slots).toEqual({ fast: ["keep"] });
  });

  it("clearing every built-in slot while keeping a custom slot still sends a defined models object", async () => {
    vi.spyOn(customSlotsApi, "get").mockResolvedValue([{ name: "fast", match: "contains" }]);
    const { result } = renderHook(() => useForm());
    act(() => {
      prepare(result);
      result.current.setEditing({ id: 7, platform_type: "openai", models: { default: ["a"], sonnet: ["b"] } } as any);
      result.current.setShowForm(true);
    });
    await waitFor(() => expect(result.current.customSlotDefs).not.toBeNull());
    act(() => {
      result.current.setModels({ default: [], sonnet: [], opus: [], haiku: [], gpt: [], jev: [], embedding: [], rerank: [] });
      result.current.setCustomSlotValues({ fast: ["f1"] });
    });
    await act(() => result.current.handleSave());
    const arg = (platformApi.update as any).mock.calls[0][0];
    expect(arg.models).toBeDefined();
    expect(Object.values(arg.models).filter((v: any) => v && v.length > 0)).toEqual([]);
    expect(JSON.parse(arg.extra).custom_slots).toEqual({ fast: ["f1"] });
  });
});

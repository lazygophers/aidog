// 票 02：编辑透传平台回填 base_url —— handleEdit 在 endpoints 空时把顶层 base_url
// 回填进 endpoints[0]，使表单 Base URL 输入框（绑定 endpoints[0]?.base_url）显示已保存值。
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useRef } from "react";
import { platformApi, groupDetailApi } from "../../services/api";
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

const passPlatform = (over: Partial<any> = {}) => ({
  id: 7,
  name: "P",
  platform_type: "passthrough",
  base_url: "https://relay.example.com",
  api_key: "k",
  endpoints: [],
  models: {},
  ...over,
} as any);

describe("handleEdit backfills base_url for passthrough platforms", () => {
  beforeEach(() => {
    vi.spyOn(platformApi, "update").mockResolvedValue(undefined as any);
    vi.spyOn(groupDetailApi, "list").mockResolvedValue([] as any);
    vi.spyOn(console, "error").mockImplementation(() => {});
  });
  afterEach(() => vi.restoreAllMocks());

  it("backfills top-level base_url into endpoints[0] when endpoints empty", async () => {
    const { result } = renderHook(() => useForm());
    await act(() => result.current.handleEdit(passPlatform()));
    expect(result.current.endpoints[0]?.base_url).toBe("https://relay.example.com");
    expect(result.current.endpoints[0]?.protocol).toBe("passthrough");
  });

  it("fills empty endpoints[0].base_url keeping its protocol", async () => {
    const { result } = renderHook(() => useForm());
    await act(() => result.current.handleEdit(passPlatform({
      endpoints: [{ protocol: "passthrough", base_url: "", client_type: "default", coding_plan: false }],
    })));
    expect(result.current.endpoints[0]?.base_url).toBe("https://relay.example.com");
    expect(result.current.endpoints[0]?.protocol).toBe("passthrough");
  });

  it("leaves non-empty endpoints untouched", async () => {
    const ep = { protocol: "passthrough", base_url: "https://keep.example.com", client_type: "default", coding_plan: false };
    const { result } = renderHook(() => useForm());
    await act(() => result.current.handleEdit(passPlatform({ endpoints: [ep] })));
    expect(result.current.endpoints).toEqual([ep]);
  });

  it("does not backfill for non-passthrough protocols", async () => {
    const { result } = renderHook(() => useForm());
    await act(() => result.current.handleEdit(passPlatform({ platform_type: "openai", endpoints: [] })));
    expect(result.current.endpoints).toEqual([]);
  });

  it("save round trip: base_url stays equal, single endpoint, no drift", async () => {
    const { result } = renderHook(() => useForm());
    await act(() => result.current.handleEdit(passPlatform()));
    await act(() => result.current.handleSave());
    expect(platformApi.update).toHaveBeenCalledWith(expect.objectContaining({
      base_url: "https://relay.example.com",
    }));
    const arg = vi.mocked(platformApi.update).mock.calls[0][0] as any;
    expect(arg.endpoints).toHaveLength(1);
    expect(arg.endpoints[0].base_url).toBe("https://relay.example.com");
  });
});

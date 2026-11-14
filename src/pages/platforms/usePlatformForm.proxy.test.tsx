// usePlatformForm 的「使用代理」开关（issue #64 T03）：编辑回填、保存序列化、新建预填。
// 桩阶段（无回填/预填/序列化）这些断言应为红；实现后转绿。
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useRef } from "react";
import { platformApi, groupDetailApi } from "../../services/api";
import { customSlotsApi } from "../../services/api/customSlots";
import { usePlatformForm } from "./usePlatformForm";

// 新建平台预填依赖的全局代理地址（真实现走 IPC，测试注入）
const proxyHost = vi.hoisted(() => ({ configured: true }));
vi.mock("../../domains/platforms/proxyConfig", async (importOriginal) => {
  const orig = await importOriginal<typeof import("../../domains/platforms/proxyConfig")>();
  return { ...orig, getProxyHostConfigured: () => Promise.resolve(proxyHost.configured) };
});

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

const plat = (extra: string) => ({
  id: 7, name: "P7", platform_type: "openai", base_url: "https://x.test/v1", api_key: "k",
  extra, models: {}, available_models: [], endpoints: [], status: "enabled",
} as never as Parameters<typeof usePlatformForm>[0]["platforms"][number]);

describe("usePlatformForm 使用代理（extra.proxy_enabled）", () => {
  beforeEach(() => {
    vi.spyOn(platformApi, "update").mockResolvedValue(undefined as any);
    vi.spyOn(platformApi, "create").mockResolvedValue(undefined as any);
    vi.spyOn(groupDetailApi, "list").mockResolvedValue([] as any);
    vi.spyOn(customSlotsApi, "get").mockResolvedValue([] as any);
    vi.spyOn(console, "error").mockImplementation(() => {});
  });
  afterEach(() => vi.restoreAllMocks());

  it("获取模型时传递当前 proxy_enabled", async () => {
    const fetchModels = vi.spyOn(platformApi, "fetchModels").mockResolvedValue(["gpt-4"]);
    const { result } = renderHook(() => useForm());
    act(() => {
      result.current.setApiKey("k");
      result.current.setEndpoints([{ protocol: "openai", base_url: "https://x.test/v1" }] as any);
      result.current.setProxyEnabled(true);
    });
    await act(() => result.current.handleFetchModels());
    expect(fetchModels).toHaveBeenCalledWith("openai", "https://x.test/v1", "k", true);
  });

  it("编辑回填：extra.proxy_enabled=true → 表单开关 true；缺失 → false", async () => {
    const { result } = renderHook(() => useForm());
    await act(() => result.current.handleEdit(plat(JSON.stringify({ keep: 1, proxy_enabled: true }))));
    expect(result.current.proxyEnabled).toBe(true);
    await act(() => result.current.handleEdit(plat("{}")));
    expect(result.current.proxyEnabled).toBe(false);
  });

  it("保存写回：编辑态保存把 proxy_enabled 序列化进 extra（保留其余键）", async () => {
    const { result } = renderHook(() => useForm());
    await act(() => result.current.handleEdit(plat(JSON.stringify({ keep: 1, proxy_enabled: true }))));
    await act(() => result.current.handleSave());
    expect(platformApi.update).toHaveBeenCalledOnce();
    const saved = JSON.parse(vi.mocked(platformApi.update).mock.calls[0][0].extra!);
    expect(saved.proxy_enabled).toBe(true);
    expect(saved.keep).toBe(1);
  });

  it("保存写回：关闭开关保存 → extra 落显式 false", async () => {
    const { result } = renderHook(() => useForm());
    await act(() => result.current.handleEdit(plat(JSON.stringify({ proxy_enabled: true }))));
    act(() => result.current.setProxyEnabled(false));
    await act(() => result.current.handleSave());
    const saved = JSON.parse(vi.mocked(platformApi.update).mock.calls[0][0].extra!);
    expect(saved.proxy_enabled).toBe(false);
  });

  it("新建预填：全局代理地址非空 → 开关默认开；为空 → 默认关（一次性写入 state）", async () => {
    proxyHost.configured = true;
    const a = renderHook(() => useForm());
    await act(() => a.result.current.openCreatePlatform());
    expect(a.result.current.proxyEnabled).toBe(true);
    a.unmount();

    proxyHost.configured = false;
    const b = renderHook(() => useForm());
    await act(() => b.result.current.openCreatePlatform());
    expect(b.result.current.proxyEnabled).toBe(false);
    b.unmount();
  });

  it("新建保存：预填值随 create 落库", async () => {
    proxyHost.configured = true;
    const { result } = renderHook(() => useForm());
    await act(() => result.current.openCreatePlatform());
    act(() => result.current.setName("New"));
    await act(() => result.current.handleSave());
    expect(platformApi.create).toHaveBeenCalledOnce();
    const saved = JSON.parse(vi.mocked(platformApi.create).mock.calls[0][0].extra ?? "{}");
    expect(saved.proxy_enabled).toBe(true);
  });
});

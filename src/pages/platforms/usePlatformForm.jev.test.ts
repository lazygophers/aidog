// usePlatformForm 的 jev 决策槽：保存 payload 剔除空白槽、一键填充不进 jev。
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useRef } from "react";
import { platformApi } from "../../services/api";
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

describe("unknown protocol repair", () => {
  beforeEach(() => {
    vi.spyOn(platformApi, "create").mockResolvedValue(undefined as any);
    vi.spyOn(platformApi, "update").mockResolvedValue(undefined as any);
    vi.spyOn(console, "error").mockImplementation(() => {});
  });
  afterEach(() => vi.restoreAllMocks());

  it("does not submit an unknown protocol", async () => {
    const { result } = renderHook(() => useForm());
    act(() => result.current.setProtocol("unknown"));
    await act(() => result.current.handleSave());
    expect(platformApi.create).not.toHaveBeenCalled();
    expect(platformApi.update).not.toHaveBeenCalled();
    expect(result.current.saveError).toContain("请选择");
  });

  it("blocks unknown protocol before batch submission", async () => {
    const { result } = renderHook(() => useForm());
    act(() => {
      result.current.setProtocol("unknown");
      result.current.setBatchPreviewKeys(["key1", "key2"]);
    });
    await act(() => result.current.handleSave());
    expect(platformApi.create).not.toHaveBeenCalled();
    expect(result.current.batchPreviewKeys).toEqual(["key1", "key2"]);
    expect(result.current.saveError).toContain("请选择");
  });

  it("repairs only the protocol without replacing existing configuration", async () => {
    const { result } = renderHook(() => useForm());
    act(() => {
      result.current.setEditing({ id: 7, platform_type: "unknown" } as any);
      result.current.setProtocol("unknown");
      result.current.setName("My platform");
      result.current.setEndpoints([{ protocol: "openai", base_url: "https://example.test/v1" }] as any);
      result.current.setModels({ default: "custom-model", sonnet: "", opus: "", haiku: "", gpt: "", jev: "" });
      result.current.setQuotaCustomScript("custom script");
    });
    await act(() => result.current.handleProtocolChange("openai"));
    await act(() => result.current.handleProtocolChange("gemini"));
    expect(result.current.protocol).toBe("gemini");
    expect(result.current.endpoints[0]?.base_url).toBe("https://example.test/v1");
    expect(result.current.models.default).toBe("custom-model");
    expect(result.current.quotaCustomScript).toBe("custom script");
    await act(() => result.current.handleSave());
    expect(platformApi.update).toHaveBeenCalledWith(expect.objectContaining({ id: 7, platform_type: "gemini", name: "My platform" }));
  });
});

describe("usePlatformForm jev slot", () => {
  it("buildModelsPayload keeps jev, drops blank slots", () => {
    const { result } = renderHook(() => useForm());
    act(() => {
      result.current.setModels({ default: "", sonnet: "  ", opus: "", haiku: "", gpt: "", jev: " jev-latest " });
    });
    expect(result.current.buildModelsPayload()).toEqual({
      default: undefined, sonnet: undefined, opus: undefined, haiku: undefined, gpt: undefined,
      jev: "jev-latest",
    });
  });

  it("buildModelsPayload is undefined when every slot (incl. jev) is blank", () => {
    const { result } = renderHook(() => useForm());
    act(() => {
      result.current.setModels({ default: "", sonnet: "", opus: "", haiku: "", gpt: "", jev: "   " });
    });
    expect(result.current.buildModelsPayload()).toBeUndefined();
  });

  it("handleFillAll fills chat slots but not jev", () => {
    const { result } = renderHook(() => useForm());
    act(() => {
      result.current.setModels({ default: "glm-5", sonnet: "", opus: "", haiku: "", gpt: "", jev: "" });
    });
    act(() => { result.current.handleFillAll(); });
    expect(result.current.models).toEqual({
      default: "glm-5", sonnet: "glm-5", opus: "glm-5", haiku: "glm-5", gpt: "glm-5", jev: "",
    });
  });
});

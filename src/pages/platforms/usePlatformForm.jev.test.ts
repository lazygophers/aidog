// usePlatformForm 的 jev 决策槽：保存 payload 剔除空白槽、一键填充不进 jev。
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useRef } from "react";
import { platformApi } from "../../services/api";
import { mockIPC } from "@tauri-apps/api/mocks";
import { __resetDefaultsCacheForTests } from "../../domains/platforms/defaults";
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

describe("protocol changes", () => {
  it("preserves filled model chains and fills empty slots from the new preset", async () => {
    __resetDefaultsCacheForTests();
    mockIPC((cmd: string) => cmd === "get_defaults_json" ? JSON.stringify({
      protocols: { gemini: { endpoints: { default: [] }, models: { default: { default: ["preset"], sonnet: ["preset-sonnet"] } }, model_list: { default: [] } } },
    }) : null);
    const { result } = renderHook(() => useForm());
    act(() => result.current.setModels({ default: ["custom", "fallback"], sonnet: [], opus: [], haiku: [], gpt: [], jev: [], embedding: [], rerank: [], image: [] }));
    await act(() => result.current.handleProtocolChange("gemini"));
    expect(result.current.models.default).toEqual(["custom", "fallback"]);
    expect(result.current.models.sonnet).toEqual(["preset-sonnet"]);
    __resetDefaultsCacheForTests();
  });
});

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
    expect(result.current.saveError).toContain("Select a valid protocol");
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
    expect(result.current.saveError).toContain("Select a valid protocol");
  });

  it("repairs only the protocol without replacing existing configuration", async () => {
    const { result } = renderHook(() => useForm());
    act(() => {
      result.current.setEditing({ id: 7, platform_type: "unknown" } as any);
      result.current.setProtocol("unknown");
      result.current.setName("My platform");
      result.current.setEndpoints([{ protocol: "openai", base_url: "https://example.test/v1" }] as any);
      result.current.setModels({ default: ["custom-model"], sonnet: [], opus: [], haiku: [], gpt: [], jev: [], embedding: [], rerank: [], image: [] });
      result.current.setQuotaCustomScript("custom script");
    });
    await act(() => result.current.handleProtocolChange("openai"));
    await act(() => result.current.handleProtocolChange("gemini"));
    expect(result.current.protocol).toBe("gemini");
    expect(result.current.endpoints[0]?.base_url).toBe("https://example.test/v1");
    expect(result.current.models.default).toEqual(["custom-model"]);
    expect(result.current.quotaCustomScript).toBe("custom script");
    await act(() => result.current.handleSave());
    expect(platformApi.update).toHaveBeenCalledWith(expect.objectContaining({ id: 7, platform_type: "gemini", name: "My platform" }));
  });
});

describe("timemodel paste and save", () => {
  afterEach(() => vi.restoreAllMocks());

  it("fullShare hydrates timemodel draft from share extra through save (票 04)", async () => {
    vi.spyOn(platformApi, "create").mockResolvedValue(undefined as any);
    const { result } = renderHook(() => useForm());
    // 预置用户草稿：fullShare 以分享串 extra 为准，水合覆盖既有草稿（票 04 前不回填 → 保存丢配置）。
    act(() => {
      result.current.timemodels.setPeak([{ start_hour: 1, end_hour: 2, multiplier: 3 }]);
      result.current.timemodels.setDisableDuringPeak(false);
      result.current.timemodels.setTimeModels([{ windows: [{ start_hour: 3, end_hour: 4, multiplier: 1 }], models: { default: "draft" } }]);
      result.current.timemodels.setWindowsTz("utc");
    });
    const sharedPeak = [{ start_hour: 9, end_hour: 10, multiplier: 2 }];
    const sharedRules = [{ windows: [{ start_hour: 8, end_hour: 12, multiplier: 1 }], models: { default: "shared" } }];
    await act(() => result.current.applyPaste({
      platform: null, baseUrls: [], apiKeys: [],
      fullShare: {
        name: "Shared", platform_type: "openai", api_key: "key", models: {},
        extra: JSON.stringify({ keep: "shared", peak: sharedPeak, disable_during_peak: true, time_windows: sharedRules }),
      } as any,
    }));
    expect(result.current.timemodels.peak).toEqual(sharedPeak);
    expect(result.current.timemodels.timeModels).toEqual(sharedRules);
    expect(result.current.timemodels.disableDuringPeak).toBe(true);
    await act(() => result.current.handleSave());
    expect(platformApi.create).toHaveBeenCalledOnce();
    const saved = JSON.parse(vi.mocked(platformApi.create).mock.calls[0][0].extra!);
    expect(saved.keep).toBe("shared");
    expect(saved.peak).toEqual(sharedPeak);
    expect(saved.time_windows).toEqual(sharedRules);
    expect(saved.disable_during_peak).toBe(true);
    // 保存成功 resetForm 后草稿清空
    expect(result.current.timemodels.peak).toEqual([]);
    expect(result.current.timemodels.timeModels).toEqual([]);
    expect(result.current.timemodels.disableDuringPeak).toBe(false);
    expect(result.current.timemodels.windowsTz).toBe("local");
  });
});

describe("usePlatformForm jev slot", () => {
  it("buildModelsPayload keeps jev, drops blank slots", () => {
    const { result } = renderHook(() => useForm());
    act(() => {
      result.current.setModels({ default: [], sonnet: [], opus: [], haiku: [], gpt: [], jev: ["jev-latest"], embedding: [], rerank: [], image: [] });
    });
    expect(result.current.buildModelsPayload()).toEqual({
      default: undefined, sonnet: undefined, opus: undefined, haiku: undefined, gpt: undefined,
      jev: ["jev-latest"], embedding: undefined, rerank: undefined, image: undefined,
    });
  });

  // 全空必须返回显式空对象（而非 undefined）：undefined 会被 JSON 丢键，后端 update 视为「保留旧值」，清空保存不生效。
  it("buildModelsPayload is an object with no filled slot when every slot (incl. jev) is blank", () => {
    const { result } = renderHook(() => useForm());
    act(() => {
      result.current.setModels({ default: [], sonnet: [], opus: [], haiku: [], gpt: [], jev: [], embedding: [], rerank: [], image: [] });
    });
    const payload = result.current.buildModelsPayload();
    expect(payload).toBeDefined();
    expect(Object.values(payload!).filter(v => v && v.length > 0)).toEqual([]);
  });

  it("handleFillAll fills chat slots but not jev", () => {
    const { result } = renderHook(() => useForm());
    act(() => {
      result.current.setModels({ default: ["glm-5"], sonnet: [], opus: [], haiku: [], gpt: [], jev: [], embedding: [], rerank: [], image: [] });
    });
    act(() => { result.current.handleFillAll(); });
    expect(result.current.models).toEqual({
      default: ["glm-5"], sonnet: ["glm-5"], opus: ["glm-5"], haiku: ["glm-5"], gpt: ["glm-5"], jev: [],
      embedding: [], rerank: [], image: [],
    });
  });
});

describe("usePlatformForm embedding / rerank slots", () => {
  it("buildModelsPayload keeps trimmed embedding / rerank", () => {
    const { result } = renderHook(() => useForm());
    act(() => {
      result.current.setModels({ default: [], sonnet: [], opus: [], haiku: [], gpt: [], jev: [], embedding: ["text-embedding-v4"], rerank: ["rr"] });
    });
    expect(result.current.buildModelsPayload()).toMatchObject({ embedding: ["text-embedding-v4"], rerank: ["rr"] });
  });

  it("only embedding filled → embedding kept; all blank → still defined, no filled slot", () => {
    const { result } = renderHook(() => useForm());
    act(() => {
      result.current.setModels({ default: [], sonnet: [], opus: [], haiku: [], gpt: [], jev: [], embedding: ["e"], rerank: [], image: [] });
    });
    expect(result.current.buildModelsPayload()?.embedding).toEqual(["e"]);
    act(() => {
      result.current.setModels({ default: [], sonnet: [], opus: [], haiku: [], gpt: [], jev: [], embedding: [], rerank: [], image: [] });
    });
    const blank = result.current.buildModelsPayload();
    expect(blank).toBeDefined();
    expect(Object.values(blank!).filter(v => v && v.length > 0)).toEqual([]);
  });

  it("handleFillAll does not fill embedding / rerank", () => {
    const { result } = renderHook(() => useForm());
    act(() => {
      result.current.setModels({ default: ["glm-5"], sonnet: [], opus: [], haiku: [], gpt: [], jev: [], embedding: [], rerank: [], image: [] });
    });
    act(() => { result.current.handleFillAll(); });
    expect(result.current.models.embedding).toEqual([]);
    expect(result.current.models.rerank).toEqual([]);
    expect(result.current.models.gpt).toEqual(["glm-5"]);
  });
});

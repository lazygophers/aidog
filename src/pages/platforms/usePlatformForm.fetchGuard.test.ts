// 一键获取守卫：模型矩阵里任意槽位已填 → 获取成功后不覆写模型，给 info 提示；全空才自动填充。
import { describe, it, expect, vi, afterEach } from "vitest";
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

const BLANK = { default: [], sonnet: [], opus: [], haiku: [], gpt: [], jev: [], embedding: [], rerank: [] };

describe("fetch models guard", () => {
  afterEach(() => vi.restoreAllMocks());

  it("any slot filled → fetch success keeps models, updates candidates, shows info", async () => {
    vi.spyOn(platformApi, "fetchModels").mockResolvedValue(["fetched-a", "fetched-b"]);
    const { result } = renderHook(() => useForm());
    act(() => {
      result.current.setApiKey("k");
      result.current.setEndpoints([{ protocol: "openai", base_url: "https://example.test/v1" }] as any);
      result.current.setModels({ ...BLANK, default: ["hand-written"] });
    });
    await act(() => result.current.handleFetchModels());
    expect(result.current.models.default).toEqual(["hand-written"]);
    expect(result.current.availableModels).toEqual(["fetched-a", "fetched-b"]);
    expect(result.current.fetchInfo).not.toBe("");
  });

  it("all slots blank → fetch success auto-fills models, no info", async () => {
    vi.spyOn(platformApi, "fetchModels").mockResolvedValue(["fetched-a", "fetched-b"]);
    const { result } = renderHook(() => useForm());
    act(() => {
      result.current.setApiKey("k");
      result.current.setEndpoints([{ protocol: "openai", base_url: "https://example.test/v1" }] as any);
      result.current.setModels({ ...BLANK });
    });
    await act(() => result.current.handleFetchModels());
    expect(result.current.models.default.length).toBeGreaterThan(0);
    expect(result.current.fetchInfo).toBe("");
  });
});

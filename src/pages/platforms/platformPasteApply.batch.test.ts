// 票 13 回归护栏：批量创建（runBatchCreateFromPaste）走 buildSharedCreateFields 共享表单草稿，
//   每个 key 一个平台、仅 name/api_key 不同。PlatformPasteCtx 缩减（删 fullShare 专用 setter）
//   不得影响本路径。观察口径：hook 公开 API → platformApi.create payload。
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useRef } from "react";
import { platformApi } from "../../services/api";
import { mockIPC } from "@tauri-apps/api/mocks";
import { __resetDefaultsCacheForTests } from "../../domains/platforms/defaults";
import { usePlatformForm } from "./usePlatformForm";
import type { Platform } from "../../services/api";

function useForm() {
  const platformsEpochRef = useRef(0);
  const consumedEditPidRef = useRef<number | null>(null);
  const deps = {
    t: ((k: string, f?: string) => f ?? k) as any,
    platforms: [],
    setPlatforms: vi.fn(),
    platformsEpochRef,
    quota: { scheduleQuotaFor: vi.fn() } as any,
    setGroupDetails: vi.fn(),
    setToast: vi.fn(),
    breakerDefaults: {} as any,
    setUsageMap: vi.fn(),
    setLastTestMap: vi.fn(),
    onNavigate: vi.fn(),
    consumedEditPidRef,
  };
  return usePlatformForm(deps as any);
}

describe("runBatchCreateFromPaste — 共享草稿批量创建（票 13 回归护栏）", () => {
  beforeEach(() => {
    __resetDefaultsCacheForTests();
    vi.spyOn(platformApi, "create").mockResolvedValue({ id: 1 } as unknown as Platform);
    mockIPC((cmd: string) => {
      if (cmd === "get_defaults_json") return JSON.stringify({ protocols: {} });
      if (cmd === "group_detail_list") return [];
      if (cmd === "settings_get") return null;
      return null;
    });
  });
  afterEach(() => { __resetDefaultsCacheForTests(); vi.restoreAllMocks(); });

  it("两个 key → 两个平台共享表单草稿配置（extra 含草稿键），仅 name/api_key 不同", async () => {
    const { result } = renderHook(() => useForm());
    await act(async () => {
      result.current.setName("Base");
      result.current.setEndpoints([{ protocol: "openai", base_url: "https://api.example/v1", client_type: "default", coding_plan: false }]);
      result.current.setExtra(JSON.stringify({ keep: "x", peak: [{ start_hour: 1, end_hour: 2, multiplier: 2 }] }));
    });
    await act(() => result.current.runBatchCreateFromPaste(["abcd1234", "efgh5678"]));
    expect(platformApi.create).toHaveBeenCalledTimes(2);
    const calls = vi.mocked(platformApi.create).mock.calls;
    expect(calls[0][0].name).toBe("Base-1234");
    expect(calls[1][0].name).toBe("Base-5678");
    expect(calls[0][0].api_key).toBe("abcd1234");
    expect(calls[1][0].api_key).toBe("efgh5678");
    const saved1 = JSON.parse((calls[0][0] as { extra?: string }).extra!);
    // 未知键原样保留（不变量①）；草稿空序列化键（peak=[] 删键，空值删键语义）不回归差异
    expect(saved1.keep).toBe("x");
    const saved2 = JSON.parse((calls[1][0] as { extra?: string }).extra!);
    expect(saved2).toEqual(saved1);
    // base_url 来自表单草稿 endpoints（共享字段）
    expect(calls[0][0].base_url).toBe("https://api.example/v1");
    expect(calls[1][0].base_url).toBe("https://api.example/v1");
  });
});

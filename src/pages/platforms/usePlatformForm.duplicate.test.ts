// 票 03（issue #58）：handleDuplicate 与 handleEdit 对同一源平台水合一致。
// D1 透传平台 endpoints 回填；D2 quota_source 取源平台值并停用自动落位。
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useRef } from "react";
import { mockIPC } from "@tauri-apps/api/mocks";
import { __resetDefaultsCacheForTests } from "../../domains/platforms/defaults";
import { usePlatformForm } from "./usePlatformForm";
import type { Platform } from "../../services/api";

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

const passthroughPlatform = {
  id: 9, name: "relay", platform_type: "passthrough", base_url: "https://x.example/v1",
  extra: "{}", models: {}, available_models: [], endpoints: [],
  status: "enabled", manual_budgets: [],
} as never as Platform;

const manualQuotaPlatform = {
  id: 11, name: "ds", platform_type: "deepseek", base_url: "https://api.example.com/v1",
  extra: "{}", models: {}, available_models: [], endpoints: [],
  status: "enabled", quota_source: "manual", manual_budgets: [{ id: "b", kind: "total", unit: "usd", amount: 10, consumed: 0, enabled: true, window_hours: null, window_unit: "hour", window_start_at: null }],
} as never as Platform;

describe("handleDuplicate 水合与 handleEdit 一致（票 03）", () => {
  beforeEach(() => {
    __resetDefaultsCacheForTests();
    // deepseek 注入配额脚本变体 → 无显式落位时新建态默认 auto（复现 D2 前提）
    mockIPC((cmd: string) => cmd === "get_defaults_json" ? JSON.stringify({
      protocols: { deepseek: { endpoints: { default: [] }, models: {}, model_list: { default: [] }, quota_scripts: [{ id: "official", requires: [{ key: "token" }] }] } },
    }) : null);
  });
  afterEach(() => { __resetDefaultsCacheForTests(); vi.restoreAllMocks(); });

  it("D1: 复制仅顶层 base_url 的透传平台 → endpoints[0].base_url 回填", async () => {
    const { result } = renderHook(() => useForm());
    await act(() => result.current.handleDuplicate(passthroughPlatform));
    expect(result.current.endpoints[0]?.base_url).toBe("https://x.example/v1");
  });

  it("D2: 复制 quota_source=manual 平台 → quotaSource=manual 且不再自动落位", async () => {
    const { result } = renderHook(() => useForm());
    await act(() => result.current.handleDuplicate(manualQuotaPlatform));
    expect(result.current.quotaSource).toBe("manual");
    // 不再自动落位：协议重选触发变体 effect 后仍保持 manual（prestine 已关）
    await act(() => result.current.handleProtocolChange("deepseek"));
    expect(result.current.quotaSource).toBe("manual");
  });
});

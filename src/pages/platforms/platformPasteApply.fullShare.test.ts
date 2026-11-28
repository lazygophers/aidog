// 票 04（issue #58）：fullShare 分享串导入后，extra 全部配置经保存落新平台（不漏水）。
// 观察口径：applyPaste 灌表单 → handleSave → platformApi.create payload.extra。
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
  const groupsReloadRef = useRef<(() => void) | null>(null);
  const consumedEditPidRef = useRef<number | null>(null);
  const deps = {
    t: ((k: string, f?: string) => f ?? k) as any,
    platforms: [],
    setPlatforms: vi.fn(),
    platformsEpochRef,
    quota: { scheduleQuotaFor: vi.fn() } as any,
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

// 分享串 extra：票 04 已定 12 类 + 既有 4 类（mock/quota/devin/breaker）。数值形状对齐
// parse* 的真实读取口径（rate_limits 带 rules、time_windows 带 windows/models）。
const shareExtra = JSON.stringify({
  rate_limits: { rules: [{ models: ["m1"], window: "60s", limit: 10 }] },
  time_windows: [{ windows: [{ start_hour: 9, end_hour: 18 }], models: { default: "w1" } }],
  peak: [{ start_hour: 14, end_hour: 18, multiplier: 3 }],
  disable_during_peak: true,
  direct_models: ["dm1"],
  allowed_clients: ["claude_code"],
  client_ua_rules: [{ kind: "prefix", pattern: "claude" }],
  mitm_stats: true,
  custom_slots: { team: ["a1"] },
  disabled_slots: ["image"],
  appbuilder_key: "abk-123",
  auth_header: "X-Team",
  auth_template: "Bearer {api_key}",
  proxy_enabled: true,
  breaker: { failure_threshold: 5, half_open_max: 2 },
});

const share = {
  name: "shared", platform_type: "passthrough", api_key: "sk-share",
  endpoints: [{ protocol: "passthrough", base_url: "https://relay.example/v1", client_type: "default", coding_plan: false }],
  models: { default: ["m1"] }, available_models: [], manual_budgets: [],
  extra: shareExtra,
};

// 断言这些键保存后仍在 payload.extra（票 04 验收口径：键在，非逐值恒等）
const mustSurvive = [
  "rate_limits", "time_windows", "peak", "disable_during_peak", "direct_models",
  "allowed_clients", "client_ua_rules", "mitm_stats", "disabled_slots",
  "auth_header", "auth_template", "proxy_enabled",
];

describe("fullShare 导入：extra 配置保存后不丢（票 04）", () => {
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

  it("导入含 12 类配置的分享串 → handleSave payload.extra 保留全部键", async () => {
    const { result } = renderHook(() => useForm());
    await act(() => result.current.applyPaste({ fullShare: share } as never));
    await act(() => result.current.handleSave());
    expect(platformApi.create).toHaveBeenCalled();
    const payload = vi.mocked(platformApi.create).mock.calls[0][0] as { extra?: string };
    expect(payload.extra).toBeTruthy();
    const saved = JSON.parse(payload.extra!);
    for (const k of mustSurvive) expect(saved[k], `key ${k} 丢失`).toBeDefined();
  });
});

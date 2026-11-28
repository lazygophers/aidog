// 票 05（issue #58）：extra 全键往返 + 三入口水合等价的契约锁定（票 06 重构护栏）。
// 只经 usePlatformForm 公开接口观察：handleEdit / handleDuplicate / applyPaste(fullShare) /
// handleSave payload；不依赖 setter 名、内部 state 形状或票 06 的新 module —— 票 12-14
// 把表单内部换成 reducer 前后，本文件必须都绿。
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useRef } from "react";
import { platformApi } from "../../services/api";
import { mockIPC } from "@tauri-apps/api/mocks";
import { __resetDefaultsCacheForTests } from "../../domains/platforms/defaults";
import { usePlatformForm } from "./usePlatformForm";
import type { Platform, SharePlatform } from "../../services/api";

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

// ── 17 对 parse*/serialize* 对应的 extra 键 + 1 个未知键（键名字面量，兜底键名手误）。
// 形状取各 serialize 的规范输出（整数小时窗口、干净数组），保证往返恒等。
const SHARE_EXTRA = {
  // verbatim 保存（非对应协议不重写）：mock / devin / appbuilder / passthrough 认证 / 自定义槽
  mock: { a: 1 },
  devin: { devin_timeout: 60 },
  appbuilder_key: "abk-123",
  auth_header: "X-Team",
  auth_template: "Bearer {api_key}",
  custom_slots: { team: ["a1"] },
  // 全协议重写（hydrate → serialize 往返）
  quota_script_id: "voapi",
  breaker: { failure_threshold: 5, half_open_max: 2 },
  rate_limits: { rpm: 60, rules: [{ models: ["glm-4"], rpm: 10 }] },
  mitm_stats: true,
  proxy_enabled: true,
  disabled_slots: ["image"],
  time_windows: [{ windows: [{ start_hour: 8, end_hour: 12, multiplier: 1 }], models: { default: "w1" } }],
  peak: [{ start_hour: 6, end_hour: 10, multiplier: 3 }],
  direct_models: ["dm1"],
  allowed_clients: ["claude_code"],
  client_ua_rules: [{ kind: "prefix", pattern: "claude" }],
  disable_during_peak: true,
  unknown_future_key: "keep-me",
} as Record<string, unknown>;
const shareExtraStr = JSON.stringify(SHARE_EXTRA);

const endpointsOf = (protocol: string, baseUrl: string) =>
  [{ protocol, base_url: baseUrl, client_type: "default", coding_plan: false }] as never[];

const fullShareOf = (extra: string, baseUrl = "https://relay.example/v1") =>
  ({
    aidog_platform_share: 1, name: "Shared", platform_type: "deepseek",
    base_url: baseUrl, api_key: "sk-share", extra,
    models: { default: ["m1"] }, available_models: [],
    endpoints: endpointsOf("openai", baseUrl), manual_budgets: [],
  }) as unknown as SharePlatform;

const platformOf = (extra: string): Platform =>
  ({
    id: 21, name: "Src", platform_type: "deepseek", base_url: "https://relay.example/v1",
    api_key: "sk-src", extra, models: { default: ["m1"] }, available_models: [],
    endpoints: endpointsOf("openai", "https://relay.example/v1"),
    manual_budgets: [], quota_source: "manual", expires_at: 1790000000000, status: "enabled",
  }) as never as Platform;

async function savedPayload(entry: (h: ReturnType<typeof useForm>) => Promise<void>) {
  const { result } = renderHook(() => useForm());
  await act(() => entry(result.current));
  await act(() => result.current.handleSave());
  const fn = vi.mocked(platformApi.update).mock.calls.length > 0 ? platformApi.update : platformApi.create;
  const payload = vi.mocked(fn).mock.calls[0][0] as { extra?: string };
  return { result, payload };
}

beforeEach(() => {
  __resetDefaultsCacheForTests();
  vi.spyOn(platformApi, "create").mockResolvedValue({ id: 99 } as unknown as Platform);
  vi.spyOn(platformApi, "update").mockResolvedValue({ id: 21 } as unknown as Platform);
  mockIPC((cmd: string) => {
    if (cmd === "get_defaults_json") {
      return JSON.stringify({ protocols: { deepseek: { endpoints: { default: [] }, models: {}, model_list: { default: [] }, quota_scripts: [{ id: "voapi", requires: [] }] } } });
    }
    if (cmd === "group_detail_list") return [];
    if (cmd === "settings_get") return [{ name: "team", match: "contains" }]; // custom_slot_defs：含 team 槽 → custom_slots 走 serialize 往返
    return null;
  });
});
afterEach(() => { __resetDefaultsCacheForTests(); vi.restoreAllMocks(); });

describe("extra 全键往返（17 对 + 未知键）", () => {
  const entries = {
    edit: (h: ReturnType<typeof useForm>) => h.handleEdit(platformOf(shareExtraStr)),
    duplicate: (h: ReturnType<typeof useForm>) => h.handleDuplicate(platformOf(shareExtraStr)),
    fullShare: (h: ReturnType<typeof useForm>) => h.applyPaste({ fullShare: fullShareOf(shareExtraStr) } as never),
  };
  for (const [name, entry] of Object.entries(entries)) {
    it(`${name}：保存 payload.extra 逐键等于源`, async () => {
      const { payload } = await savedPayload(entry);
      expect(payload.extra).toBeTruthy();
      const saved = JSON.parse(payload.extra!) as Record<string, unknown>;
      for (const [key, expected] of Object.entries(SHARE_EXTRA)) {
        expect(saved[key], `key ${key} 不等于源`).toEqual(expected);
      }
    });
  }
});

describe("三入口非 extra 字段水合等价", () => {
  // 源平台：透传存量形态（endpoints 空、仅顶层 base_url）+ 手动配额 + 手动预算 + 过期时间。
  // SharePlatform 不携带 quota_source：fullShare 入口的 quota_source 由 manual_budgets 非空推导；
  // expires_at 自 issue #309 起随分享串携带（旧串缺字段回落 0）。
  const src: Platform = {
    id: 33, name: "Relay", platform_type: "passthrough", base_url: "https://relay.example/v1",
    api_key: "sk-src", extra: "{}", models: {}, available_models: [],
    endpoints: [], manual_budgets: [
      { id: "b", kind: "total", unit: "usd", amount: 10, consumed: 4, enabled: true, window_hours: null, window_unit: "hour", window_start_at: null },
    ] as never, quota_source: "manual", expires_at: 1790000000000, status: "enabled",
  } as never as Platform;
  const share = {
    ...fullShareOf("{}", "https://relay.example/v1"), platform_type: "passthrough",
    endpoints: [], manual_budgets: src.manual_budgets, expires_at: 1790000000000,
  } as unknown as SharePlatform;

  it("edit 与 duplicate：endpoints 透传回填 / quota_source / manual_budgets / expires_at 一致，仅 editing 不同", async () => {
    const ra = renderHook(() => useForm());
    await act(() => ra.result.current.handleEdit(src));
    const rb = renderHook(() => useForm());
    await act(() => rb.result.current.handleDuplicate(src));
    for (const r of [ra.result.current, rb.result.current]) {
      expect(r.draft.endpoints[0]?.base_url).toBe("https://relay.example/v1");
      expect(r.draft.quotaSource).toBe("manual");
      expect(r.draft.manualBudgets).toEqual(src.manual_budgets);
      expect(r.draft.expiresAt).toBe(1790000000000);
    }
    expect(ra.result.current.editing).not.toBeNull();
    expect(rb.result.current.editing).toBeNull();
  });

  it("fullShare：endpoints 透传回填 / manual_budgets / quota_source(manual) / expires_at 一致", async () => {
    const { result } = renderHook(() => useForm());
    await act(() => result.current.applyPaste({ fullShare: share } as never));
    expect(result.current.draft.endpoints[0]?.base_url).toBe("https://relay.example/v1");
    expect(result.current.draft.manualBudgets).toEqual(src.manual_budgets);
    expect(result.current.draft.quotaSource).toBe("manual");
    expect(result.current.draft.expiresAt).toBe(1790000000000);
  });
});

describe("非法 JSON extra：三入口不抛错，原串保留", () => {
  const bad = "not-json{{";
  it.each([
    ["edit", (h: ReturnType<typeof useForm>) => h.handleEdit(platformOf(bad))],
    ["duplicate", (h: ReturnType<typeof useForm>) => h.handleDuplicate(platformOf(bad))],
    ["fullShare", (h: ReturnType<typeof useForm>) => h.applyPaste({ fullShare: fullShareOf(bad) } as never)],
  ])("%s：extra 原串保留", async (_name, entry) => {
    const { result } = renderHook(() => useForm());
    await act(() => entry(result.current));
    expect(result.current.draft.extra).toBe(bad);
  });
});

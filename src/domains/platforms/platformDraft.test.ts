// 票 06（issue #58）：平台配置草稿 module 纯函数测试。
// 契约与 platformDraft.contract.test.ts（票 05，hook 侧）对齐：17 键 + 未知键逐键往返恒等、
// 空值删键、devin 先于 quota 的序列化顺序、reducer 三个 action、非法 JSON 原串保留。
import { describe, it, expect } from "vitest";
import {
  emptyDraft, draftReducer, draftFromPlatform, draftFromShare, draftToPayload,
} from "./platformDraft";
import type { Platform, SharePlatform } from "../../services/api";

// ── 17 对 parse*/serialize* 对应的 extra 键 + 1 个未知键（键名字面量，兜底键名手误）。
const SHARE_EXTRA = {
  mock: { a: 1 },
  devin: { devin_timeout: 60 },
  appbuilder_key: "abk-123",
  auth_header: "X-Team",
  auth_template: "Bearer {api_key}",
  custom_slots: { team: ["a1"] },
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

const env = {
  quotaVariants: [{ id: "voapi", requires: [] }],
  customSlotDefs: [{ name: "team", match: "contains" as const }],
};

function savedExtraOf(extra: string, entry: "edit" | "duplicate" | "share"): Record<string, unknown> {
  const draft = entry === "share"
    ? draftFromShare(fullShareOf(extra))
    : draftFromPlatform(platformOf(extra), entry);
  const payload = draftToPayload(draft, env);
  expect(payload.extra).toBeTruthy();
  return JSON.parse(payload.extra!) as Record<string, unknown>;
}

describe("extra 全键往返（17 对 + 未知键）", () => {
  for (const entry of ["edit", "duplicate", "share"] as const) {
    it(`${entry}：draftToPayload(...).extra 逐键等于源`, () => {
      const saved = savedExtraOf(shareExtraStr, entry);
      for (const [key, expected] of Object.entries(SHARE_EXTRA)) {
        expect(saved[key], `key ${key} 不等于源`).toEqual(expected);
      }
    });
  }
});

describe("空值删键", () => {
  it("空草稿序列化：17 键全删，仅剩 proxy_enabled:false", () => {
    const payload = draftToPayload(emptyDraft(), env);
    const saved = JSON.parse(payload.extra!) as Record<string, unknown>;
    expect(Object.keys(saved).sort()).toEqual(["proxy_enabled"]);
    expect(saved.proxy_enabled).toBe(false);
  });
  it("清空的时段键从存量 base 中删除", () => {
    const base = JSON.stringify({ peak: [{ start_hour: 1, end_hour: 2, multiplier: 2 }], disabled_slots: ["image"], keep_me: 1 });
    const draft = { ...draftFromPlatform(platformOf(base), "edit"), peak: [], disabledSlots: [] };
    const saved = JSON.parse(draftToPayload(draft, env).extra!) as Record<string, unknown>;
    expect("peak" in saved).toBe(false);
    expect("disabled_slots" in saved).toBe(false);
    expect(saved.keep_me).toBe(1);
  });
});

describe("序列化顺序：devin 先于 quota（org_id 镜像写 extra.devin）", () => {
  it("devin_timeout 与 quota 镜像 org_id 并存", () => {
    const draft = {
      ...emptyDraft(),
      protocol: "devin" as const,
      devinConfig: { devin_timeout: 99, devin_mode: "fast" },
      quotaVariantId: "v1",
      quotaRequires: { org_id: "org7" },
    };
    const payload = draftToPayload(draft, {
      quotaVariants: [{ id: "v1", requires: [{ key: "org_id" }] as never }],
      customSlotDefs: null,
    });
    const saved = JSON.parse(payload.extra!) as { devin?: Record<string, unknown> };
    expect(saved.devin?.devin_timeout).toBe(99);
    expect(saved.devin?.org_id).toBe("org7");
    expect(saved.org_id).toBe("org7");
  });
});

describe("draftReducer", () => {
  it("patch 只覆盖给出的键", () => {
    const s = { ...emptyDraft(), name: "A", protocol: "openai" as const };
    const next = draftReducer(s, { type: "patch", patch: { name: "B" } });
    expect(next.name).toBe("B");
    expect(next.protocol).toBe("openai");
    expect(s.name).toBe("A"); // 纯函数：不改入参
  });
  it("load 整体替换", () => {
    const d = { ...emptyDraft(), name: "X" };
    expect(draftReducer(emptyDraft(), { type: "load", draft: d })).toBe(d);
  });
  it("reset 等价 load(emptyDraft())", () => {
    const s = { ...emptyDraft(), name: "Y", expiresAt: 5 };
    expect(draftReducer(s, { type: "reset" })).toEqual(emptyDraft());
  });
});

describe("非法 JSON extra", () => {
  it("原串保留在 draft.extra，序列化不抛错", () => {
    const bad = "not-json{{";
    const draft = draftFromPlatform(platformOf(bad), "edit");
    expect(draft.extra).toBe(bad);
    const payload = draftToPayload(draft, env);
    expect(typeof payload.extra).toBe("string");
    expect(() => JSON.parse(payload.extra!)).not.toThrow();
  });
});

describe("三入口非 extra 字段水合等价（module 层）", () => {
  const src: Platform = {
    id: 33, name: "Relay", platform_type: "passthrough", base_url: "https://relay.example/v1",
    api_key: "sk-src", extra: "{}", models: {}, available_models: [],
    endpoints: [], manual_budgets: [
      { id: "b", kind: "total", unit: "usd", amount: 10, consumed: 4, enabled: true, window_hours: null, window_unit: "hour", window_start_at: null },
    ] as never, quota_source: "manual", expires_at: 1790000000000, status: "enabled",
  } as never as Platform;
  const share = {
    ...fullShareOf("{}", "https://relay.example/v1"), platform_type: "passthrough",
    endpoints: [], manual_budgets: src.manual_budgets,
  } as unknown as SharePlatform;

  it("edit 与 duplicate 字段一致，仅 sourceId 不同", () => {
    const a = draftFromPlatform(src, "edit");
    const b = draftFromPlatform(src, "duplicate");
    expect(a.endpoints[0]?.base_url).toBe("https://relay.example/v1");
    expect(a.quotaSource).toBe("manual");
    expect(a.manualBudgets).toEqual(src.manual_budgets);
    expect(a.expiresAt).toBe(1790000000000);
    expect(a.sourceId).toBe(33);
    expect(b.sourceId).toBeNull();
    const { sourceId: _a, ...restA } = a;
    const { sourceId: _b, ...restB } = b;
    expect(restA).toEqual(restB);
  });
  it("fullShare：透传回填 / quota_source(manual 推导) / manual_budgets 一致", () => {
    const d = draftFromShare(share);
    expect(d.endpoints[0]?.base_url).toBe("https://relay.example/v1");
    expect(d.quotaSource).toBe("manual");
    expect(d.manualBudgets).toEqual(src.manual_budgets);
    expect(d.sourceId).toBeNull();
  });
  it("fullShare 无预算 → quotaSource auto", () => {
    expect(draftFromShare(fullShareOf("{}")).quotaSource).toBe("auto");
  });
});

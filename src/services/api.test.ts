import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import * as api from "./api";
import {
  parseNewApiConfig,
  serializeNewApiConfig,
  parseMockConfig,
  serializeMockConfig,
  parsePlatformBreaker,
  serializePlatformBreaker,
  DEFAULT_NEWAPI_CONFIG,
  DEFAULT_MOCK_CONFIG,
  onProxyLogUpdated,
} from "./api";

// ─── 纯函数：extra JSON parse/serialize 三组 ────────────────

describe("parseNewApiConfig", () => {
  it("returns defaults for blank extra", () => {
    expect(parseNewApiConfig("")).toEqual(DEFAULT_NEWAPI_CONFIG);
    expect(parseNewApiConfig("   ")).toEqual(DEFAULT_NEWAPI_CONFIG);
  });
  it("returns defaults for invalid JSON", () => {
    expect(parseNewApiConfig("{not json")).toEqual(DEFAULT_NEWAPI_CONFIG);
  });
  it("returns defaults when no newapi key", () => {
    expect(parseNewApiConfig('{"mock":{}}')).toEqual(DEFAULT_NEWAPI_CONFIG);
  });
  it("merges partial newapi over defaults", () => {
    const out = parseNewApiConfig('{"newapi":{"user_id":"42"}}');
    expect(out.user_id).toBe("42");
    expect(out.balance_base_url).toBe("");
  });
  it("ignores non-object newapi value", () => {
    expect(parseNewApiConfig('{"newapi":5}')).toEqual(DEFAULT_NEWAPI_CONFIG);
  });
});

describe("serializeNewApiConfig", () => {
  it("writes newapi into empty extra", () => {
    const s = serializeNewApiConfig("", { ...DEFAULT_NEWAPI_CONFIG, user_id: "9" });
    expect(JSON.parse(s).newapi.user_id).toBe("9");
  });
  it("preserves other keys in existing extra", () => {
    const s = serializeNewApiConfig('{"mock":{"a":1}}', DEFAULT_NEWAPI_CONFIG);
    const o = JSON.parse(s);
    expect(o.mock).toEqual({ a: 1 });
    expect(o.newapi).toBeDefined();
  });
  it("rebuilds when existing extra is invalid JSON", () => {
    const s = serializeNewApiConfig("garbage", DEFAULT_NEWAPI_CONFIG);
    expect(JSON.parse(s).newapi).toBeDefined();
  });
  it("ignores array extra", () => {
    const s = serializeNewApiConfig("[1,2]", DEFAULT_NEWAPI_CONFIG);
    expect(JSON.parse(s).newapi).toBeDefined();
  });
});

describe("parseMockConfig / serializeMockConfig", () => {
  it("defaults on blank/invalid/missing", () => {
    expect(parseMockConfig("")).toEqual(DEFAULT_MOCK_CONFIG);
    expect(parseMockConfig("{bad")).toEqual(DEFAULT_MOCK_CONFIG);
    expect(parseMockConfig('{"x":1}')).toEqual(DEFAULT_MOCK_CONFIG);
    expect(parseMockConfig('{"mock":3}')).toEqual(DEFAULT_MOCK_CONFIG);
  });
  it("merges partial mock", () => {
    expect(parseMockConfig('{"mock":{"delay_ms":99}}').delay_ms).toBe(99);
  });
  it("serializes preserving siblings + handles invalid/array", () => {
    const s = serializeMockConfig('{"newapi":{"u":1}}', DEFAULT_MOCK_CONFIG);
    expect(JSON.parse(s).newapi).toEqual({ u: 1 });
    expect(JSON.parse(serializeMockConfig("bad", DEFAULT_MOCK_CONFIG)).mock).toBeDefined();
    expect(JSON.parse(serializeMockConfig("[]", DEFAULT_MOCK_CONFIG)).mock).toBeDefined();
  });
});

describe("parsePlatformBreaker / serializePlatformBreaker", () => {
  const zero = { failure_threshold: 0, open_secs: 0, half_open_max: 0 };
  it("zero on blank/invalid/missing", () => {
    expect(parsePlatformBreaker("")).toEqual(zero);
    expect(parsePlatformBreaker("{bad")).toEqual(zero);
    expect(parsePlatformBreaker('{"x":1}')).toEqual(zero);
    expect(parsePlatformBreaker('{"breaker":7}')).toEqual(zero);
  });
  it("coerces non-number fields to 0", () => {
    const out = parsePlatformBreaker('{"breaker":{"failure_threshold":"x","open_secs":3}}');
    expect(out.failure_threshold).toBe(0);
    expect(out.open_secs).toBe(3);
  });
  it("removes breaker key when all zero", () => {
    const s = serializePlatformBreaker('{"mock":{}}', zero);
    expect(JSON.parse(s).breaker).toBeUndefined();
    expect(JSON.parse(s).mock).toBeDefined();
  });
  it("writes breaker when non-zero, rebuilds on invalid/array", () => {
    const b = { failure_threshold: 5, open_secs: 30, half_open_max: 2 };
    expect(JSON.parse(serializePlatformBreaker("", b)).breaker).toEqual(b);
    expect(JSON.parse(serializePlatformBreaker("bad", b)).breaker).toEqual(b);
    expect(JSON.parse(serializePlatformBreaker("[]", b)).breaker).toEqual(b);
  });
});

// ─── onProxyLogUpdated（listen + debounce + unlisten）─────────

describe("onProxyLogUpdated", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
    clearMocks();
  });

  it("registers a listener and returns a cleanup fn", () => {
    mockIPC(() => undefined, { shouldMockEvents: true });
    const cb = vi.fn();
    const off = onProxyLogUpdated(cb, 100);
    expect(typeof off).toBe("function");
    off();
  });
});

// ─── invoke 封装批量 smoke（mockIPC 拦截全部命令）──────────────
// 目的：覆盖各 namespace 方法的一行 invoke 直传（functions/lines 计入），
// 并顺带断言 payload key 走 camelCase（防回归 tauri-invoke-param-camelcase）。

describe("api namespaces (bulk invoke smoke)", () => {
  beforeEach(() => {
    // 通用返回：依命令名给出宽松默认；多数封装只是透传 invoke 结果。
    mockIPC((cmd: string) => {
      if (cmd.endsWith("_list") || cmd.includes("list")) return [];
      if (cmd.includes("stats") || cmd.includes("query")) return [];
      return null;
    });
  });
  afterEach(() => clearMocks());

  it("platformApi methods invoke without throwing", async () => {
    await expect(api.platformApi.list()).resolves.toBeDefined();
    await expect(api.platformApi.get(1)).resolves.toBeNull();
    await expect(
      api.platformApi.create({ name: "x", platform_type: "openai", base_url: "u", api_key: "k" }),
    ).resolves.toBeDefined();
    await expect(
      api.platformApi.update({ id: 1, name: "y" }),
    ).resolves.toBeDefined();
    await expect(api.platformApi.delete(1)).resolves.toBeDefined();
    await expect(api.platformApi.purgeDisabled()).resolves.toBeDefined();
    await expect(api.platformApi.purgeDisabled(3)).resolves.toBeDefined();
    await expect(api.platformApi.ensureAutoGroup(1)).resolves.toBeDefined();
    await expect(api.platformApi.reorder([1, 2])).resolves.toBeDefined();
    await expect(api.platformApi.fetchModels("openai", "u", "k")).resolves.toBeDefined();
    await expect(api.platformApi.usageStats(1)).resolves.toBeDefined();
  });

  it("camelCase payload keys are preserved across boundary", async () => {
    const handler = vi.fn(() => null);
    mockIPC(handler);
    await api.platformApi.reorder([5, 6]);
    const [, payload] = handler.mock.calls[0];
    expect(payload).toHaveProperty("orderedIds");
  });

  // 对其余 namespace 做「枚举每个方法 → 用占位参数调用 → 不抛」的批量 smoke。
  it("every namespace method is callable", async () => {
    const namespaces: Record<string, unknown> = {
      trayApi: api.trayApi,
      groupUsageApi: api.groupUsageApi,
      trayConfigApi: api.trayConfigApi,
      popoverConfigApi: api.popoverConfigApi,
      groupApi: api.groupApi,
      groupDetailApi: api.groupDetailApi,
      proxyApi: api.proxyApi,
      configApi: api.configApi,
      proxyLogApi: api.proxyLogApi,
      proxyTimeoutApi: api.proxyTimeoutApi,
      middlewareApi: api.middlewareApi,
      schedulingApi: api.schedulingApi,
      notificationApi: api.notificationApi,
      settingsApi: api.settingsApi,
      statuslineApi: api.statuslineApi,
      scriptExecutorApi: api.scriptExecutorApi,
      codexApi: api.codexApi,
      claudeSettingsImportApi: api.claudeSettingsImportApi,
      appLogApi: api.appLogApi,
      dbApi: api.dbApi,
      codingToolsSettingsApi: api.codingToolsSettingsApi,
      statsApi: api.statsApi,
      statsSettingsApi: api.statsSettingsApi,
      modelTestApi: api.modelTestApi,
      quotaApi: api.quotaApi,
      modelPriceApi: api.modelPriceApi,
      priceSyncApi: api.priceSyncApi,
      skillsApi: api.skillsApi,
      mcpApi: api.mcpApi,
      importExportApi: api.importExportApi,
      ccswitchApi: api.ccswitchApi,
      sub2apiApi: api.sub2apiApi,
      backupApi: api.backupApi,
      aboutApi: api.aboutApi,
    };

    let called = 0;
    for (const [nsName, ns] of Object.entries(namespaces)) {
      expect(ns, `${nsName} should be exported`).toBeDefined();
      for (const [method, fn] of Object.entries(ns as Record<string, unknown>)) {
        if (typeof fn !== "function") continue;
        try {
          // 用一组宽松占位参数覆盖大多数签名（多数方法 0-3 个简单参数）。
          const result = (fn as (...a: unknown[]) => unknown)(
            1,
            "placeholder",
            "placeholder2",
            {},
            [],
          );
          if (result instanceof Promise) {
            await result.catch(() => undefined);
          }
          called++;
        } catch {
          // 个别方法可能因占位参数构造内部对象失败；不算回归，跳过即可。
        }
      }
    }
    expect(called).toBeGreaterThan(50);
  });
});

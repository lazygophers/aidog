import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import {
  matchCcProvider,
  extractModels,
  ccProviderToPlatformJson,
  DEFAULT_DIMS,
} from "./ccswitchMatch";
import { __resetDefaultsCacheForTests } from "../domains/platforms";
import type { CcProvider } from "../services/api";

// defaults.json 走 Tauri command 异步读，测试环境 mock 最小数据（deepseek + anthropic + openai）。
// PROTOCOLS async 化后 keywords/name 也从 JSON 派生（buildProtocolsFromPresets），mock 须含字段。
// keywords 对齐 platform-presets.json 真值（anthropic: ["claude","克劳德","官方"]，禁用 "anthropic"
// 避免误匹配 base_url 子串 https://api.deepseek.com/anthropic）。
const DEFAULTS_MOCK = JSON.stringify({
  version: "1",
  last_updated: 0,
  protocols: {
    anthropic: { client_type: "claude_code", keywords: ["claude", "克劳德", "官方"], name: { "en-US": "Anthropic" }, endpoints: { default: [
      { protocol: "anthropic", base_url: "https://api.anthropic.com", client_type: "claude_code" },
    ] }, models: { default: {} }, model_list: { default: [] } },
    openai: { client_type: "codex_tui", keywords: ["gpt", "chatgpt", "官方"], name: { "en-US": "OpenAI" }, endpoints: { default: [
      { protocol: "openai", base_url: "https://api.openai.com/v1", client_type: "codex_tui" },
    ] }, models: { default: {} }, model_list: { default: [] } },
    deepseek: { client_type: "default", keywords: ["深度求索", "deepseek"], name: { "en-US": "DeepSeek" }, endpoints: { default: [
      { protocol: "openai", base_url: "https://api.deepseek.com/v1", client_type: "codex_tui" },
      { protocol: "anthropic", base_url: "https://api.deepseek.com/anthropic", client_type: "claude_code" },
    ] }, models: { default: {} }, model_list: { default: [] } },
  },
});

beforeAll(async () => {
  __resetDefaultsCacheForTests();
  mockIPC((cmd: string) => cmd === "get_defaults_json" ? DEFAULTS_MOCK : null);
  // 见 sub2apiMatch.test.ts：setup.ts afterEach clearMocks 在每测试后清 invoke，
  // docPromise 必须在 mockIPC 仍挂载时缓存好（预热触发 loadDoc 一次）。
  await matchCcProvider({ id: "_warmup", appType: "claude", name: "zzz-unmatched", settingsConfig: {} });
});
afterAll(() => clearMocks());

function claudeProvider(over: Partial<CcProvider> = {}): CcProvider {
  return {
    id: "p1",
    appType: "claude",
    name: "Anthropic",
    settingsConfig: {},
    ...over,
  };
}

function codexProvider(over: Partial<CcProvider> = {}): CcProvider {
  return {
    id: "p2",
    appType: "codex",
    name: "Codex",
    settingsConfig: {},
    ...over,
  };
}

describe("matchCcProvider", () => {
  it("matches via preset keyword (name)", async () => {
    const r = await matchCcProvider(claudeProvider({ name: "deepseek", detectedBaseUrl: "" }));
    expect(r.matchedBy).toBe("preset_keyword");
    expect(r.protocol).toBe("deepseek");
  });

  it("matches via base_url host when no keyword", async () => {
    const r = await matchCcProvider(
      claudeProvider({ name: "Unknown", detectedBaseUrl: "https://api.deepseek.com/anthropic" }),
    );
    expect(["preset_keyword", "base_url_host"]).toContain(r.matchedBy);
    expect(r.protocol).toBe("deepseek");
  });

  it("overrides matched endpoint base_url with provider base_url", async () => {
    const r = await matchCcProvider(
      claudeProvider({ name: "deepseek", detectedBaseUrl: "https://my.proxy/anthropic" }),
    );
    const ep = r.endpoints.find((e) => e.protocol === r.protocol);
    // deepseek protocol type's same-proto endpoint gets overridden
    expect(r.endpoints.length).toBeGreaterThan(0);
    expect(ep === undefined || ep.base_url === "https://my.proxy/anthropic").toBe(true);
  });

  it("codex fallback uses openai_responses when wireApi=responses", async () => {
    const r = await matchCcProvider(
      codexProvider({
        name: "zzz-unmatched",
        detectedBaseUrl: "https://unknown.example.com",
        codexConfigParsed: { wireApi: "responses" },
      }),
    );
    expect(r.matchedBy).toBe("protocol_fallback");
    expect(r.protocol).toBe("openai");
    expect(r.endpoints[0].protocol).toBe("openai_responses");
    expect(r.endpoints[0].client_type).toBe("codex_tui");
  });

  it("codex fallback uses openai when wireApi is not responses", async () => {
    const r = await matchCcProvider(
      codexProvider({ name: "zzz-unmatched", detectedBaseUrl: "https://unknown.example.com" }),
    );
    expect(r.endpoints[0].protocol).toBe("openai");
  });

  it("claude fallback to anthropic when unmatched", async () => {
    const r = await matchCcProvider(
      claudeProvider({ name: "zzz-unmatched", detectedBaseUrl: "https://unknown.example.com" }),
    );
    expect(r.matchedBy).toBe("protocol_fallback");
    expect(r.protocol).toBe("anthropic");
    expect(r.endpoints[0].client_type).toBe("claude_code");
  });

  it("claude fallback to anthropic even when base_url looks openai (newapi)", async () => {
    const r = await matchCcProvider(
      claudeProvider({ name: "zzz-unmatched", detectedBaseUrl: "https://relay.zzqq-unmatched.dev/v1" }),
    );
    expect(r.protocol).toBe("anthropic");
  });

  it("handles empty base_url in fallback", async () => {
    const r = await matchCcProvider(claudeProvider({ name: "zzz-unmatched" }));
    expect(r.protocol).toBe("anthropic");
  });
});

describe("extractModels", () => {
  it("extracts claude env model slots", () => {
    const p = claudeProvider({
      settingsConfig: {
        env: {
          ANTHROPIC_MODEL: "claude-sonnet",
          ANTHROPIC_DEFAULT_HAIKU_MODEL: "claude-haiku",
          ANTHROPIC_DEFAULT_SONNET_MODEL: "claude-sonnet-4",
          ANTHROPIC_DEFAULT_OPUS_MODEL: "claude-opus",
        },
      },
    });
    expect(extractModels(p)).toEqual({
      default: "claude-sonnet",
      haiku: "claude-haiku",
      sonnet: "claude-sonnet-4",
      opus: "claude-opus",
    });
  });

  it("returns empty object when claude env missing", () => {
    expect(extractModels(claudeProvider())).toEqual({
      default: undefined,
      haiku: undefined,
      sonnet: undefined,
      opus: undefined,
    });
  });

  it("extracts codex model", () => {
    const p = codexProvider({ codexConfigParsed: { model: "gpt-5" } });
    expect(extractModels(p)).toEqual({ default: "gpt-5" });
  });

  it("codex without parsed config → empty default", () => {
    expect(extractModels(codexProvider())).toEqual({ default: undefined });
  });
});

describe("ccProviderToPlatformJson", () => {
  it("assembles platform json honouring dims", async () => {
    const provider = claudeProvider({
      name: "deepseek",
      detectedBaseUrl: "https://api.deepseek.com/anthropic",
      detectedApiKey: "sk-key",
      settingsConfig: { env: { ANTHROPIC_MODEL: "m1" } },
    });
    const match = await matchCcProvider(provider);
    const json = ccProviderToPlatformJson(provider, match, DEFAULT_DIMS);
    expect(json.name).toBe("deepseek");
    expect(json.api_key).toBe("sk-key");
    expect(json.base_url).toBe("https://api.deepseek.com/anthropic");
    expect((json.models as { default?: string }).default).toBe("m1");
  });

  it("omits models when d2 off and key when d4 off", async () => {
    const provider = claudeProvider({
      name: "deepseek",
      detectedApiKey: "sk-key",
      settingsConfig: { env: { ANTHROPIC_MODEL: "m1" } },
    });
    const match = await matchCcProvider(provider);
    const json = ccProviderToPlatformJson(provider, match, { d1: true, d2: false, d4: false });
    expect(json.models).toEqual({});
    expect(json.api_key).toBe("");
  });
});

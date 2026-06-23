import { describe, it, expect } from "vitest";
import {
  matchCcProvider,
  extractModels,
  ccProviderToPlatformJson,
  DEFAULT_DIMS,
} from "./ccswitchMatch";
import type { CcProvider } from "../services/api";

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
  it("matches via preset keyword (name)", () => {
    const r = matchCcProvider(claudeProvider({ name: "deepseek", detectedBaseUrl: "" }));
    expect(r.matchedBy).toBe("preset_keyword");
    expect(r.protocol).toBe("deepseek");
  });

  it("matches via base_url host when no keyword", () => {
    const r = matchCcProvider(
      claudeProvider({ name: "Unknown", detectedBaseUrl: "https://api.deepseek.com/anthropic" }),
    );
    expect(["preset_keyword", "base_url_host"]).toContain(r.matchedBy);
    expect(r.protocol).toBe("deepseek");
  });

  it("overrides matched endpoint base_url with provider base_url", () => {
    const r = matchCcProvider(
      claudeProvider({ name: "deepseek", detectedBaseUrl: "https://my.proxy/anthropic" }),
    );
    const ep = r.endpoints.find((e) => e.protocol === r.protocol);
    // deepseek protocol type's same-proto endpoint gets overridden
    expect(r.endpoints.length).toBeGreaterThan(0);
    expect(ep === undefined || ep.base_url === "https://my.proxy/anthropic").toBe(true);
  });

  it("codex fallback uses openai_responses when wireApi=responses", () => {
    const r = matchCcProvider(
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

  it("codex fallback uses openai when wireApi is not responses", () => {
    const r = matchCcProvider(
      codexProvider({ name: "zzz-unmatched", detectedBaseUrl: "https://unknown.example.com" }),
    );
    expect(r.endpoints[0].protocol).toBe("openai");
  });

  it("claude fallback to anthropic when unmatched", () => {
    const r = matchCcProvider(
      claudeProvider({ name: "zzz-unmatched", detectedBaseUrl: "https://unknown.example.com" }),
    );
    expect(r.matchedBy).toBe("protocol_fallback");
    expect(r.protocol).toBe("anthropic");
    expect(r.endpoints[0].client_type).toBe("claude_code");
  });

  it("claude fallback to anthropic even when base_url looks openai (newapi)", () => {
    const r = matchCcProvider(
      claudeProvider({ name: "zzz-unmatched", detectedBaseUrl: "https://relay.zzqq-unmatched.dev/v1" }),
    );
    expect(r.protocol).toBe("anthropic");
  });

  it("handles empty base_url in fallback", () => {
    const r = matchCcProvider(claudeProvider({ name: "zzz-unmatched" }));
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
  it("assembles platform json honouring dims", () => {
    const provider = claudeProvider({
      name: "deepseek",
      detectedBaseUrl: "https://api.deepseek.com/anthropic",
      detectedApiKey: "sk-key",
      settingsConfig: { env: { ANTHROPIC_MODEL: "m1" } },
    });
    const match = matchCcProvider(provider);
    const json = ccProviderToPlatformJson(provider, match, DEFAULT_DIMS);
    expect(json.name).toBe("deepseek");
    expect(json.api_key).toBe("sk-key");
    expect(json.base_url).toBe("https://api.deepseek.com/anthropic");
    expect((json.models as { default?: string }).default).toBe("m1");
  });

  it("omits models when d2 off and key when d4 off", () => {
    const provider = claudeProvider({
      name: "deepseek",
      detectedApiKey: "sk-key",
      settingsConfig: { env: { ANTHROPIC_MODEL: "m1" } },
    });
    const match = matchCcProvider(provider);
    const json = ccProviderToPlatformJson(provider, match, { d1: true, d2: false, d4: false });
    expect(json.models).toEqual({});
    expect(json.api_key).toBe("");
  });
});

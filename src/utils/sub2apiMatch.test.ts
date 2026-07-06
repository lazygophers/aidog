import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import {
  mapPlatformToProtocol,
  sub2apiAccountToPlatformJson,
} from "./sub2apiMatch";
import { __resetDefaultsCacheForTests } from "../domains/platforms";
import type { Sub2ApiAccount } from "../services/api";

// defaults.json 走 Tauri command 异步读，测试环境 mock 最小数据（anthropic/openai/gemini）。
const DEFAULTS_MOCK = JSON.stringify({
  version: "1",
  last_updated: 0,
  protocols: {
    anthropic: { client_type: "claude_code", endpoints: { default: [
      { protocol: "anthropic", base_url: "https://api.anthropic.com", client_type: "claude_code" },
    ] }, models: { default: {} }, model_list: { default: [] } },
    openai: { client_type: "codex_tui", endpoints: { default: [
      { protocol: "openai", base_url: "https://api.openai.com/v1", client_type: "codex_tui" },
    ] }, models: { default: {} }, model_list: { default: [] } },
    gemini: { client_type: "default", endpoints: { default: [
      { protocol: "gemini", base_url: "https://generativelanguage.googleapis.com" },
    ] }, models: { default: {} }, model_list: { default: [] } },
  },
});

beforeAll(async () => {
  __resetDefaultsCacheForTests();
  mockIPC((cmd: string) => cmd === "get_defaults_json" ? DEFAULTS_MOCK : null);
  // setup.ts 的 afterEach 在每测试后 clearMocks() 删 invoke，docPromise 必须在 mockIPC 仍挂载时
  // 缓存好（生产里 doc 在首次调用 4 函数时异步拉，测试环境 here-and-now 预热）。
  // 通过 sub2apiAccountToPlatformJson 触发 loadDoc 一次，docPromise 进程缓存好后后续测试直接命中。
  await sub2apiAccountToPlatformJson({ name: "_warmup", platform: "anthropic" });
});
afterAll(() => clearMocks());

describe("mapPlatformToProtocol", () => {
  it("maps known platform values directly", () => {
    expect(mapPlatformToProtocol("anthropic")).toEqual({
      protocol: "anthropic",
      recognized: true,
    });
    expect(mapPlatformToProtocol("openai")).toEqual({ protocol: "openai", recognized: true });
    expect(mapPlatformToProtocol("gemini")).toEqual({ protocol: "gemini", recognized: true });
  });
  it("normalizes case and whitespace", () => {
    expect(mapPlatformToProtocol("  Anthropic  ")).toEqual({
      protocol: "anthropic",
      recognized: true,
    });
  });
  it("falls back to openai (unrecognized)", () => {
    expect(mapPlatformToProtocol("weird")).toEqual({ protocol: "openai", recognized: false });
  });
});

describe("sub2apiAccountToPlatformJson", () => {
  const account: Sub2ApiAccount = {
    name: "My Acct",
    platform: "anthropic",
    apiKey: "sk-ant-xxx",
    baseUrl: "https://custom.example.com",
  };

  it("builds a platform json shape with mapped protocol", async () => {
    const json = await sub2apiAccountToPlatformJson(account);
    expect(json.name).toBe("My Acct");
    expect(json.platform_type).toBe("anthropic");
    expect(json.api_key).toBe("sk-ant-xxx");
    expect(json.base_url).toBe("https://custom.example.com");
    expect(json.enabled).toBe(true);
    expect(json.status).toBe("enabled");
    expect(Array.isArray(json.endpoints)).toBe(true);
  });

  it("overrides same-protocol endpoint base_url with provided base_url", async () => {
    const json = await sub2apiAccountToPlatformJson(account);
    const eps = json.endpoints as Array<{ protocol: string; base_url: string }>;
    const anthropicEp = eps.find((e) => e.protocol === "anthropic");
    expect(anthropicEp?.base_url).toBe("https://custom.example.com");
  });

  it("honours protocolOverride", async () => {
    const json = await sub2apiAccountToPlatformJson(account, "openai");
    expect(json.platform_type).toBe("openai");
  });

  it("falls back to preset base_url when none provided", async () => {
    const noUrl: Sub2ApiAccount = { name: "X", platform: "anthropic" };
    const json = await sub2apiAccountToPlatformJson(noUrl);
    expect(typeof json.base_url).toBe("string");
    expect((json.base_url as string).length).toBeGreaterThan(0);
    expect(json.api_key).toBe("");
  });
});

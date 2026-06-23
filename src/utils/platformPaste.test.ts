// @vitest-environment node
import { describe, it, expect } from "vitest";
import {
  normalizeForMatch,
  guessProtocol,
  matchPlatform,
  parsePlatformPaste,
  type PastePresetRef,
} from "./platformPaste";

const PRESETS: PastePresetRef[] = [
  { value: "deepseek", label: "DeepSeek", keywords: ["deepseek"], hosts: ["api.deepseek.com"] },
  {
    value: "glm",
    label: "GLM",
    keywords: ["glm", "智谱"],
    hosts: ["open.bigmodel.cn/api/paas/v4"],
  },
  {
    value: "glm_coding",
    label: "GLM Coding",
    keywords: ["glm coding"],
    hosts: ["open.bigmodel.cn/api/coding"],
    codingPlan: true,
  },
  {
    value: "xiaomi_mimo",
    label: "Xiaomi MiMo",
    keywords: ["xiaomi", "mimo"],
    hosts: ["api.xiaomimimo.com"],
  },
  {
    value: "xiaomi_mimo_coding",
    label: "Xiaomi MiMo Coding",
    keywords: [],
    hosts: ["token-plan-cn.xiaomimimo.com"],
    codingPlan: true,
  },
  { value: "mock", label: "Mock", keywords: ["测试", "mock"] },
];

describe("normalizeForMatch", () => {
  it("lowercases, replaces non-alnum/CJK with space, folds whitespace", () => {
    expect(normalizeForMatch("Hello,  WORLD!")).toBe("hello world");
    expect(normalizeForMatch("  GLM-4.5  ")).toBe("glm 4 5");
  });
  it("keeps CJK chars", () => {
    expect(normalizeForMatch("智谱AI")).toBe("智谱ai");
  });
});

describe("guessProtocol", () => {
  it("detects anthropic (with truncation tolerance)", () => {
    expect(guessProtocol("https://api.anthropic.com")).toBe("anthropic");
    expect(guessProtocol("https://x.com/anthropi")).toBe("anthropic");
  });
  it("detects gemini", () => {
    expect(guessProtocol("https://generativelanguage.googleapis.com")).toBe("gemini");
    expect(guessProtocol("https://x/gemini")).toBe("gemini");
  });
  it("detects openai (incl. /v1 path)", () => {
    expect(guessProtocol("https://api.openai.com")).toBe("openai");
    expect(guessProtocol("https://host/v1")).toBe("openai");
  });
  it("returns unknown for unrecognized", () => {
    expect(guessProtocol("https://example.com/foo")).toBe("unknown");
  });
});

describe("matchPlatform", () => {
  it("matches by base_url host (longest/most specific wins)", () => {
    const hit = matchPlatform("", PRESETS, [
      { url: "https://token-plan-cn.xiaomimimo.com/v1", protocol: "openai" },
    ]);
    expect(hit?.value).toBe("xiaomi_mimo_coding");
    expect(hit?.codingPlan).toBe(true);
  });
  it("distinguishes same-host coding vs normal by path substring", () => {
    const coding = matchPlatform("", PRESETS, [
      { url: "https://open.bigmodel.cn/api/coding/paas/v4", protocol: "openai" },
    ]);
    expect(coding?.value).toBe("glm_coding");
    const normal = matchPlatform("", PRESETS, [
      { url: "https://open.bigmodel.cn/api/paas/v4", protocol: "openai" },
    ]);
    expect(normal?.value).toBe("glm");
  });
  it("falls back to keyword scan when no host match", () => {
    const hit = matchPlatform("使用 deepseek 模型", PRESETS);
    expect(hit?.value).toBe("deepseek");
  });
  it("excludes NEVER_AUTO_MATCH presets (mock) even on keyword hit", () => {
    expect(matchPlatform("跑个测试", PRESETS)).toBeNull();
  });
  it("returns null when nothing matches", () => {
    expect(matchPlatform("random gibberish text", PRESETS)).toBeNull();
  });
});

describe("parsePlatformPaste", () => {
  it("returns empty result for blank text", () => {
    expect(parsePlatformPaste("", PRESETS)).toEqual({
      apiKeys: [],
      baseUrls: [],
      platform: null,
      models: [],
    });
    expect(parsePlatformPaste("   ", PRESETS).platform).toBeNull();
  });

  it("extracts prefix-anchored api keys", () => {
    const out = parsePlatformPaste(
      "key: sk-ant-abcdefghijklmnop1234 base https://api.anthropic.com",
      PRESETS,
    );
    expect(out.apiKeys.some((k) => k.startsWith("sk-ant-"))).toBe(true);
    expect(out.baseUrls.some((b) => b.url.includes("anthropic"))).toBe(true);
  });

  it("strips anti-crawl CJK chars injected into a key", () => {
    const out = parsePlatformPaste("apikey: sk-请删除这些字proj1234567890abcd", PRESETS);
    expect(out.apiKeys.length).toBeGreaterThan(0);
    expect(out.apiKeys[0]).not.toMatch(/[一-鿿]/);
  });

  it("decodes base64-encoded keys via assignment anchor", () => {
    // base64("sk-decoded-key-1234567890") → assign value, no known prefix
    const b64 = Buffer.from("sk-decoded-key-1234567890").toString("base64");
    const out = parsePlatformPaste(`API_KEY（base64编码）: ${b64}`, PRESETS);
    expect(out.apiKeys).toContain("sk-decoded-key-1234567890");
  });

  it("extracts base_url, dedups, skips image assets", () => {
    const out = parsePlatformPaste(
      "url https://api.deepseek.com/v1 logo https://cdn.x/logo.png again https://api.deepseek.com/v1",
      PRESETS,
    );
    const urls = out.baseUrls.map((b) => b.url);
    expect(urls).toContain("https://api.deepseek.com/v1");
    expect(urls.some((u) => u.endsWith(".png"))).toBe(false);
    // dedup
    expect(urls.filter((u) => u === "https://api.deepseek.com/v1").length).toBe(1);
  });

  it("matches platform from extracted base_url", () => {
    const out = parsePlatformPaste("接口地址 https://api.deepseek.com/v1", PRESETS);
    expect(out.platform?.value).toBe("deepseek");
  });

  it("extracts bare base64 key (no label) when decoded shape is key-like", () => {
    // base64 of "tt-bareKeyAbcdefghij1234567890" (matches DECODED_KEY_SHAPE tt-...)
    const b64 = Buffer.from("tt-barekeyabcdefghij1234567890").toString("base64");
    const out = parsePlatformPaste(`随便一段文字 ${b64} 结尾`, PRESETS);
    expect(out.apiKeys).toContain("tt-barekeyabcdefghij1234567890");
  });

  it("rejoins anti-crawl CJK split bare base64 then decodes", () => {
    const full = Buffer.from("sk-joinedkeyabcdefghij1234567890").toString("base64");
    const cut = Math.floor(full.length / 2);
    // 在 base64 串中间插入 CJK 噪声指令
    const injected = `${full.slice(0, cut)}删掉我再base64解码${full.slice(cut)}`;
    const out = parsePlatformPaste(injected, PRESETS);
    expect(out.apiKeys.some((k) => k.startsWith("sk-joinedkey"))).toBe(true);
  });

  it("assignment-anchored plaintext long key without known prefix is kept", () => {
    const out = parsePlatformPaste("token: ABCDEFGHIJKLMNOPQRSTUVWX1234", PRESETS);
    expect(out.apiKeys).toContain("ABCDEFGHIJKLMNOPQRSTUVWX1234");
  });

  it("parses base64-decoded chinese compound label string", () => {
    const compound = "令牌sk-cmp-abcdefghijklmnop地址https://api.deepseek.com/v1模型deepseek-chat";
    const b64 = Buffer.from(compound, "utf-8").toString("base64");
    const out = parsePlatformPaste(b64, PRESETS);
    expect(out.apiKeys.some((k) => k.startsWith("sk-cmp-"))).toBe(true);
    expect(out.baseUrls.some((b) => b.url.includes("deepseek"))).toBe(true);
    expect(out.models.length).toBeGreaterThan(0);
  });
});

import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import { getProtocolUsageWarning, __resetDefaultsCacheForTests } from "./defaults";
import type { Protocol } from "../../services/api";

const DOC = JSON.stringify({
  last_updated: 0,
  protocols: {
    tencent_coding: {
      usage_warning: { "en-US": "EN warn", "zh-Hans": "ZH 警示", "ja-JP": "JA warn" },
    },
    deepseek: {},
  },
});

beforeAll(async () => {
  __resetDefaultsCacheForTests();
  mockIPC((cmd: string) => (cmd === "get_defaults_json" ? DOC : null));
  await getProtocolUsageWarning("deepseek" as Protocol);
});
afterAll(() => clearMocks());

describe("getProtocolUsageWarning", () => {
  it("按 locale 返回对应文案", async () => {
    expect(await getProtocolUsageWarning("tencent_coding" as Protocol, "zh-Hans")).toBe("ZH 警示");
    expect(await getProtocolUsageWarning("tencent_coding" as Protocol, "ja-JP")).toBe("JA warn");
  });
  it("缺该 locale 回落 en-US", async () => {
    expect(await getProtocolUsageWarning("tencent_coding" as Protocol, "fr-FR")).toBe("EN warn");
  });
  it("未配置的平台返空串", async () => {
    expect(await getProtocolUsageWarning("deepseek" as Protocol, "zh-Hans")).toBe("");
  });
});

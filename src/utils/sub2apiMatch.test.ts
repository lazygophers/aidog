import { describe, it, expect } from "vitest";
import {
  mapPlatformToProtocol,
  sub2apiAccountToPlatformJson,
} from "./sub2apiMatch";
import type { Sub2ApiAccount } from "../services/api";

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

  it("builds a platform json shape with mapped protocol", () => {
    const json = sub2apiAccountToPlatformJson(account);
    expect(json.name).toBe("My Acct");
    expect(json.platform_type).toBe("anthropic");
    expect(json.api_key).toBe("sk-ant-xxx");
    expect(json.base_url).toBe("https://custom.example.com");
    expect(json.enabled).toBe(true);
    expect(json.status).toBe("enabled");
    expect(Array.isArray(json.endpoints)).toBe(true);
  });

  it("overrides same-protocol endpoint base_url with provided base_url", () => {
    const json = sub2apiAccountToPlatformJson(account);
    const eps = json.endpoints as Array<{ protocol: string; base_url: string }>;
    const anthropicEp = eps.find((e) => e.protocol === "anthropic");
    expect(anthropicEp?.base_url).toBe("https://custom.example.com");
  });

  it("honours protocolOverride", () => {
    const json = sub2apiAccountToPlatformJson(account, "openai");
    expect(json.platform_type).toBe("openai");
  });

  it("falls back to preset base_url when none provided", () => {
    const noUrl: Sub2ApiAccount = { name: "X", platform: "anthropic" };
    const json = sub2apiAccountToPlatformJson(noUrl);
    expect(typeof json.base_url).toBe("string");
    expect((json.base_url as string).length).toBeGreaterThan(0);
    expect(json.api_key).toBe("");
  });
});

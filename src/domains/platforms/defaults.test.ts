import { describe, it, expect, beforeAll, beforeEach, afterAll } from "vitest";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import {
  getDefaultModels,
  getDefaultModelList,
  getDefaultEndpoints,
  buildProtocolsFromPresets,
  __resetDefaultsCacheForTests,
} from "./defaults";
import type { Protocol } from "../../services/api";

/** 最小 preset mock：3 协议覆盖分支拓扑与 client_type 派生。
 *  - glm_coding: 带 models.{default,peak} 双分支，endpoints.default 2 端点（无 client_type → 派生）
 *  - kimi_coding: 仅 models.default 单分支，endpoints.default 1 端点（无 client_type → 派生）
 *  - deepseek: 仅 models.default 单分支（向后兼容），endpoint 显式 client_type 例外保留。
 */
const DEFAULTS_MOCK = JSON.stringify({
  version: "1",
  last_updated: 0,
  protocols: {
    glm_coding: {
      is_coding_plan: true,
      endpoints: { default: [
        { protocol: "openai", base_url: "https://open.bigmodel.cn/api/coding/paas/v4", coding_plan: true },
        { protocol: "anthropic", base_url: "https://open.bigmodel.cn/api/anthropic", coding_plan: true },
      ] },
      // PRD 07-11：default + peak 双分支
      models: {
        default: { default: "glm-5.2", opus: "glm-5.2", sonnet: "glm-4.7", gpt: "glm-5.2", haiku: "glm-4.5" },
        peak: { default: "glm-4.7", opus: "glm-4.7", sonnet: "glm-4.6", gpt: "glm-4.7", haiku: "glm-4.5" },
      },
      model_list: { default: ["glm-5.2", "glm-4.7", "glm-4.6", "glm-4.5"] },
      name: { "en-US": "GLM Coding" },
    },
    kimi_coding: {
      is_coding_plan: true,
      endpoints: {
        default: [{ protocol: "openai", base_url: "https://api.kimi.com/coding/v1", coding_plan: true }],
      },
      models: {
        default: { default: "kimi-default", sonnet: "kimi-sonnet-default" },
      },
      model_list: { default: ["kimi-default"] },
      name: { "en-US": "Kimi Coding", "zh-Hans": "Kimi 编程" },
      keywords: ["moonshot"],
    },
    deepseek: {
      endpoints: { default: [{ protocol: "openai", base_url: "https://api.deepseek.com/v1", client_type: "default" }] },
      // 单分支（向后兼容：无 peak 分支）
      models: { default: { default: "deepseek-v4-flash" } },
      model_list: { default: ["deepseek-v4-flash"] },
      name: { "en-US": "DeepSeek" },
    },
  },
});

beforeAll(async () => {
  __resetDefaultsCacheForTests();
  mockIPC((cmd: string) => (cmd === "get_defaults_json" ? DEFAULTS_MOCK : null));
  // 预热 docPromise：setup.ts afterEach 会 clearMocks，缓存必须在 mock 仍挂载时建立。
  await getDefaultModels("deepseek" as Protocol);
});
// buildProtocolsFromPresets 走 fetchDoc（无缓存直读），每测试都发一次 IPC：
// setup.ts afterEach 清了 mock，这里重挂。
beforeEach(() => mockIPC((cmd: string) => (cmd === "get_defaults_json" ? DEFAULTS_MOCK : null)));

afterAll(() => {
  clearMocks();
});

describe("getDefaultModels — PRD 07-11 peak 分支", () => {
  it("glm_coding 非高峰（isPeak=false/undefined）→ 返 default 分支", async () => {
    const m = await getDefaultModels("glm_coding" as Protocol);
    expect(m.sonnet).toBe("glm-4.7");
    expect(m.haiku).toBe("glm-4.5");
    expect(m.default).toBe("glm-5.2");
  });

  it("glm_coding 高峰（isPeak=true）→ 切 peak 分支", async () => {
    const m = await getDefaultModels("glm_coding" as Protocol, true);
    expect(m.sonnet).toBe("glm-4.6");
    expect(m.default).toBe("glm-4.7");
    expect(m.haiku).toBe("glm-4.5");
  });

  it("kimi_coding 无 peak 分支 + isPeak=true → 回落 default", async () => {
    // 向后兼容：preset 无 peak 分支 → isPeak=true 仍返 default
    const m = await getDefaultModels("kimi_coding" as Protocol, true);
    expect(m.default).toBe("kimi-default");
  });

  it("deepseek 单分支（无 peak）：isPeak=true 不影响，返 default", async () => {
    const m = await getDefaultModels("deepseek" as Protocol, true);
    expect(m.default).toBe("deepseek-v4-flash");
  });
});

describe("getDefaultEndpoints / getDefaultModelList — 单分支 default + client_type 派生", () => {
  it("getDefaultEndpoints 缺省 client_type 按 protocol 派生（openai→codex_tui / anthropic→claude_code）", async () => {
    const eps = await getDefaultEndpoints("glm_coding" as Protocol);
    expect(eps.length).toBe(2);
    expect(eps[0].protocol).toBe("openai");
    expect(eps[0].client_type).toBe("codex_tui");
    expect(eps[1].protocol).toBe("anthropic");
    expect(eps[1].client_type).toBe("claude_code");
  });

  it("getDefaultEndpoints 显式 client_type 例外保留（不覆盖）", async () => {
    const eps = await getDefaultEndpoints("deepseek" as Protocol);
    expect(eps[0].client_type).toBe("default");
  });

  it("getDefaultModelList glm_coding 含 glm-4.5 / glm-4.6（R4 补全）", async () => {
    const list = await getDefaultModelList("glm_coding" as Protocol);
    expect(list).toContain("glm-4.5");
    expect(list).toContain("glm-4.6");
    expect(list).toContain("glm-4.7");
  });
});

describe("buildProtocolsFromPresets — searchTerms 跨语言搜索", () => {
  it("searchTerms 含 name 全 locale + label + keywords（UI locale 无关）", async () => {
    const list = await buildProtocolsFromPresets("en-US");
    const kimi = list.find(p => p.value === ("kimi_coding" as Protocol));
    expect(kimi).toBeDefined();
    expect(kimi!.searchTerms).toContain("Kimi Coding");   // en-US label
    expect(kimi!.searchTerms).toContain("Kimi 编程");      // zh-Hans name（UI 在 en-US 也可搜中文）
    expect(kimi!.searchTerms).toContain("moonshot");       // keywords
  });

  it("name 只有单 locale 时 searchTerms 不含空串", async () => {
    const list = await buildProtocolsFromPresets("zh-Hans");
    const ds = list.find(p => p.value === ("deepseek" as Protocol));
    expect(ds!.searchTerms).toContain("DeepSeek");
    expect(ds!.searchTerms!.every(t => t.trim())).toBe(true);
  });
});

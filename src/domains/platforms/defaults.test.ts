import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import {
  getDefaultModels,
  getDefaultModelList,
  getDefaultEndpoints,
  __resetDefaultsCacheForTests,
} from "./defaults";
import type { Protocol } from "../../services/api";

/** 最小 preset mock：3 协议覆盖 3 种分支拓扑。
 *  - glm_coding: 带 models.{default,peak} 双分支（无 coding_plan 分支），endpoints.default 2 端点
 *  - kimi_coding: 带 models.{default,coding_plan} 双分支（无 peak），endpoints.default + coding_plan
 *  - deepseek: 仅 models.default 单分支（向后兼容）。
 */
const DEFAULTS_MOCK = JSON.stringify({
  version: "1",
  last_updated: 0,
  protocols: {
    glm_coding: {
      is_coding_plan: true,
      client_type: "codex_tui",
      endpoints: { default: [
        { protocol: "openai", base_url: "https://open.bigmodel.cn/api/coding/paas/v4", client_type: "codex_tui", coding_plan: true },
        { protocol: "anthropic", base_url: "https://open.bigmodel.cn/api/anthropic", client_type: "claude_code", coding_plan: true },
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
      client_type: "codex_tui",
      endpoints: {
        default: [{ protocol: "openai", base_url: "https://api.kimi.com/coding/v1", client_type: "codex_tui", coding_plan: true }],
        coding_plan: [{ protocol: "anthropic", base_url: "https://api.kimi.com/coding/anthropic", client_type: "claude_code", coding_plan: true }],
      },
      models: {
        default: { default: "kimi-default", sonnet: "kimi-sonnet-default" },
        coding_plan: { default: "kimi-cp", sonnet: "kimi-cp-sonnet" },
      },
      model_list: { default: ["kimi-default"] },
      name: { "en-US": "Kimi Coding" },
    },
    deepseek: {
      client_type: "codex_tui",
      endpoints: { default: [{ protocol: "openai", base_url: "https://api.deepseek.com/v1", client_type: "codex_tui" }] },
      // 单分支（向后兼容：无 peak / coding_plan 分支）
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
    const m = await getDefaultModels("glm_coding" as Protocol, false, true);
    expect(m.sonnet).toBe("glm-4.6");
    expect(m.default).toBe("glm-4.7");
    expect(m.haiku).toBe("glm-4.5");
  });

  it("kimi_coding codingPlan=true → coding_plan 分支（与 peak 互斥，cp 优先）", async () => {
    // coding_plan 与 peak 同时命中时 cp 优先（端点维度硬约束 > 时段维度软切换）
    const m = await getDefaultModels("kimi_coding" as Protocol, true, true);
    expect(m.default).toBe("kimi-cp");
    expect(m.sonnet).toBe("kimi-cp-sonnet");
  });

  it("kimi_coding codingPlan=true isPeak=false → coding_plan 分支", async () => {
    const m = await getDefaultModels("kimi_coding" as Protocol, true, false);
    expect(m.default).toBe("kimi-cp");
  });

  it("kimi_coding 无 cp 端点（codingPlan=false）+ isPeak=true 但无 peak 分支 → 回落 default", async () => {
    // 向后兼容：preset 无 peak 分支 → isPeak=true 仍返 default
    const m = await getDefaultModels("kimi_coding" as Protocol, false, true);
    expect(m.default).toBe("kimi-default");
  });

  it("deepseek 单分支（无 peak 无 cp）：isPeak=true 不影响，返 default", async () => {
    const m = await getDefaultModels("deepseek" as Protocol, false, true);
    expect(m.default).toBe("deepseek-v4-flash");
  });
});

describe("getDefaultEndpoints / getDefaultModelList — 仅 default/coding_plan 两分支（不含 peak）", () => {
  it("getDefaultEndpoints 不受 isPeak 影响（无第 3 参）", async () => {
    const eps = await getDefaultEndpoints("glm_coding" as Protocol);
    expect(eps.length).toBe(2);
    expect(eps[0].protocol).toBe("openai");
  });

  it("getDefaultModelList glm_coding 含 glm-4.5 / glm-4.6（R4 补全）", async () => {
    const list = await getDefaultModelList("glm_coding" as Protocol);
    expect(list).toContain("glm-4.5");
    expect(list).toContain("glm-4.6");
    expect(list).toContain("glm-4.7");
  });
});

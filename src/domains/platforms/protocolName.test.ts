// 平台展示名读取层：三层回落（name[locale] → name["en-US"] → 协议 code）与品牌外链。
// 真值源 = registry（`src-tauri/defaults/registry/platforms/<code>/platform.json`），
// 经 get_defaults_json 合并回传；此处用两组 registry fixture 断言切 locale 后名字变化。
import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import {
  getProtocolLabel,
  getProtocolLabelMap,
  getProtocolSourceUrls,
  buildProtocolsFromPresets,
  normalizeDefaultsLocale,
  __resetDefaultsCacheForTests,
} from "./defaults";
import type { Protocol } from "../../services/api";

/** registry fixture：3 协议覆盖三层回落的每一层。
 *  - glm_coding: 8 locale 齐全（zh-Hans / ja-JP 各有专名）→ 命中当前 locale
 *  - deepseek: 只有 en-US（缺 ja-JP）→ 回落 en-US
 *  - nameless: 无 name 字段 → 回落协议 code */
const REGISTRY_MOCK = JSON.stringify({
  version: "1",
  last_updated: 0,
  protocols: {
    glm_coding: {
      endpoints: { default: [] },
      models: {},
      model_list: {},
      name: {
        "en-US": "GLM Coding Plan (Zhipu AI)",
        "zh-Hans": "GLM 编码套餐（智谱）",
        "ja-JP": "GLM コーディングプラン（智譜）",
      },
      keywords: ["智谱编程", "glm coding"],
      homepage: "https://www.zhipuai.cn",
      source_urls: { docs: "https://docs.bigmodel.cn/quick-start", pricing: "https://docs.bigmodel.cn/overview" },
    },
    deepseek: {
      endpoints: { default: [] },
      models: {},
      model_list: {},
      // 空白值视同缺失（不因 "  " 显示空名）
      name: { "en-US": "DeepSeek", "ja-JP": "   " },
    },
    nameless: {
      endpoints: { default: [] },
      models: {},
      model_list: {},
    },
  },
});

beforeAll(async () => {
  __resetDefaultsCacheForTests();
  mockIPC((cmd: string) => (cmd === "get_defaults_json" ? REGISTRY_MOCK : null));
  // 预热 docPromise：setup.ts afterEach 会 clearMocks，缓存必须在 mock 仍挂载时建立。
  await getProtocolLabel("deepseek" as Protocol, "en-US");
});

afterAll(() => {
  clearMocks();
});

describe("normalizeDefaultsLocale — 与 Rust Lang::from_locale().locale_key() 对称", () => {
  it("8 locale 原样归一", () => {
    expect(normalizeDefaultsLocale("zh-Hans")).toBe("zh-Hans");
    expect(normalizeDefaultsLocale("ja-JP")).toBe("ja-JP");
    expect(normalizeDefaultsLocale("ar-SA")).toBe("ar-SA");
  });

  it("变体（zh / zh-CN / ja / zh_hans）归一到 8 key 之一", () => {
    expect(normalizeDefaultsLocale("zh")).toBe("zh-Hans");
    expect(normalizeDefaultsLocale("zh-CN")).toBe("zh-Hans");
    expect(normalizeDefaultsLocale("zh_hans")).toBe("zh-Hans");
    expect(normalizeDefaultsLocale("ja")).toBe("ja-JP");
  });

  it("未知语言 / 缺省 → en-US", () => {
    expect(normalizeDefaultsLocale("ko-KR")).toBe("en-US");
    expect(normalizeDefaultsLocale(undefined)).toBe("en-US");
  });
});

describe("getProtocolLabel — 三层回落", () => {
  it("切 locale 后平台名文本变化（zh-Hans / ja-JP 两组 fixture）", async () => {
    expect(await getProtocolLabel("glm_coding" as Protocol, "zh-Hans")).toBe("GLM 编码套餐（智谱）");
    expect(await getProtocolLabel("glm_coding" as Protocol, "ja-JP")).toBe("GLM コーディングプラン（智譜）");
    expect(await getProtocolLabel("glm_coding" as Protocol, "en-US")).toBe("GLM Coding Plan (Zhipu AI)");
  });

  it("缺当前 locale → 回落 en-US（不留空）", async () => {
    expect(await getProtocolLabel("deepseek" as Protocol, "ar-SA")).toBe("DeepSeek");
    // 空白值视同缺失
    expect(await getProtocolLabel("deepseek" as Protocol, "ja-JP")).toBe("DeepSeek");
  });

  it("name 整体缺失 / 协议不存在 → 回落协议 code", async () => {
    expect(await getProtocolLabel("nameless" as Protocol, "zh-Hans")).toBe("nameless");
    expect(await getProtocolLabel("not_in_registry" as Protocol, "zh-Hans")).toBe("not_in_registry");
  });
});

describe("getProtocolLabelMap / buildProtocolsFromPresets — 与 getProtocolLabel 同一回落链", () => {
  it("labelMap 切 locale 跟随，缺译回落 en-US，无 name 回落 code", async () => {
    const zh = await getProtocolLabelMap("zh-Hans");
    const ja = await getProtocolLabelMap("ja-JP");
    expect(zh.glm_coding).toBe("GLM 编码套餐（智谱）");
    expect(ja.glm_coding).toBe("GLM コーディングプラン（智譜）");
    expect(ja.deepseek).toBe("DeepSeek");
    expect(zh.nameless).toBe("nameless");
  });

  it("协议选择器选项 label 同源，keywords 原样透传供拼音/子串搜索", async () => {
    const opts = await buildProtocolsFromPresets("zh-Hans");
    const glm = opts.find(o => o.value === ("glm_coding" as Protocol))!;
    expect(glm.label).toBe("GLM 编码套餐（智谱）");
    expect(glm.keywords).toEqual(["智谱编程", "glm coding"]);
    expect(opts.find(o => o.value === ("nameless" as Protocol))!.label).toBe("nameless");
  });
});

describe("getProtocolSourceUrls — registry source_urls 为对象 {docs, pricing}", () => {
  it("配置齐全 → 返两条外链", async () => {
    expect(await getProtocolSourceUrls("glm_coding" as Protocol)).toEqual({
      docs: "https://docs.bigmodel.cn/quick-start",
      pricing: "https://docs.bigmodel.cn/overview",
    });
  });

  it("未配置 → 两空串（调用方据此不渲染）", async () => {
    expect(await getProtocolSourceUrls("deepseek" as Protocol)).toEqual({ docs: "", pricing: "" });
  });
});

import { describe, it, expect } from "vitest";
import { platformMatchesQuery, groupMatchesQuery } from "./query";
import type { Platform } from "../../services/api";

const p = { id: 1, name: "GLM 测试", base_url: "https://open.bigmodel.cn/api/paas/v4", platform_type: "glm" } as unknown as Platform;
const terms = { glm: ["智谱", "zhipu", "GLM-4.7", "bigmodel", "codegeex"] } as Record<string, string[]>;

describe("platformMatchesQuery — 跨语言/拼音匹配", () => {
  it("用户自填 name 直接命中（无 termsMap 也成立）", () => {
    expect(platformMatchesQuery(p, "glm 测试")).toBe(true);
    expect(platformMatchesQuery(p, "ceshi")).toBe(true); // 拼音
  });

  it("protocolTerms 命中 registry 词条（UI 语言无关）", () => {
    expect(platformMatchesQuery(p, "智谱", terms)).toBe(true);
    expect(platformMatchesQuery(p, "zhipu", terms)).toBe(true);
    expect(platformMatchesQuery(p, "bigmodel", terms)).toBe(true);
    // 中文词条的拼音形式已作为字面词条入库（platform.json keywords），纯子串命中
    expect(platformMatchesQuery(p, "zhip", terms)).toBe(true);
  });

  it("无 protocolTerms 或词条不存在 → 不误报", () => {
    expect(platformMatchesQuery(p, "智谱")).toBe(false);
    expect(platformMatchesQuery(p, "智谱", { kimi: ["moonshot"] })).toBe(false);
    expect(platformMatchesQuery(p, "kimi", terms)).toBe(false);
  });
});

describe("groupMatchesQuery — 原行为不变", () => {
  it("分组名/拼音命中", () => {
    const g = { id: 1, name: "测试组", group_key: "test" } as never;
    expect(groupMatchesQuery(g, "测试组")).toBe(true);
    expect(groupMatchesQuery(g, "ceshizu")).toBe(true);
    expect(groupMatchesQuery(g, "test")).toBe(true);
    expect(groupMatchesQuery(g, "other")).toBe(false);
  });
});

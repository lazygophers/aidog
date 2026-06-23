// @vitest-environment node
import { describe, it, expect } from "vitest";
import { pinyinMatch } from "./pinyin";

describe("pinyinMatch", () => {
  it("empty query matches everything", () => {
    expect(pinyinMatch("", "anything")).toBe(true);
    expect(pinyinMatch("   ", "anything")).toBe(true);
  });

  it("direct substring match (case-insensitive)", () => {
    expect(pinyinMatch("GLM", "GLM-4")).toBe(true);
    expect(pinyinMatch("glm", "GLM-4")).toBe(true);
    expect(pinyinMatch("lian", "百炼")).toBe(true); // matches via target pinyin "bailian"
  });

  it("full pinyin of chinese target", () => {
    expect(pinyinMatch("bailian", "百炼")).toBe(true);
    expect(pinyinMatch("bai", "百炼")).toBe(true);
    expect(pinyinMatch("xiaomi", "小米")).toBe(true);
  });

  it("chinese query converted to pinyin then matched", () => {
    // target latin, query chinese → queryPinyin path
    expect(pinyinMatch("百", "百炼")).toBe(true);
    expect(pinyinMatch("炼", "百炼")).toBe(true);
  });

  it("mixed chinese + latin query", () => {
    expect(pinyinMatch("百lian", "百炼")).toBe(true);
    expect(pinyinMatch("xiao米", "小米")).toBe(true);
  });

  it("returns false when nothing matches", () => {
    expect(pinyinMatch("zzz", "百炼")).toBe(false);
  });

  it("non-chinese characters preserved during pinyin conversion", () => {
    expect(pinyinMatch("xiaomiai", "小米AI")).toBe(true);
  });

  it("LRU cache survives many distinct targets (eviction path)", () => {
    // 构造 > 500 个不同 target 触发淘汰，确认仍正确匹配（不崩、命中正常）
    for (let i = 0; i < 600; i++) {
      pinyinMatch("x", `平台${i}`);
    }
    // 早期 target 已被淘汰，重新查询应重算且正确
    expect(pinyinMatch("pingtai", "平台0")).toBe(true);
    // 缓存命中路径：再查一次同 target
    expect(pinyinMatch("pingtai", "平台0")).toBe(true);
  });
});

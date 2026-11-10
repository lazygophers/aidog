// embedding / rerank 槽位：MODEL_SLOTS 收录、专用槽标记、allModelValues 计入。
import { describe, it, expect } from "vitest";
import { MODEL_SLOTS, DEDICATED_MODEL_SLOTS } from "./constants";
import { allModelValues, allConfiguredModelValues } from "./health";
import { autoCategorize } from "./autoCategorize";

describe("embedding / rerank model slots", () => {
  it("MODEL_SLOTS has both, with locale keys", () => {
    const m = Object.fromEntries(MODEL_SLOTS.map(s => [s.key, s.labelKey]));
    expect(m.embedding).toBe("platform.modelEmbedding");
    expect(m.rerank).toBe("platform.modelRerank");
  });

  it("DEDICATED_MODEL_SLOTS = embedding + rerank only", () => {
    expect([...DEDICATED_MODEL_SLOTS]).toEqual(["embedding", "rerank"]);
  });

  it("allModelValues includes both slots, deduped", () => {
    expect(allModelValues({ default: "a", embedding: "e", rerank: "e" })).toEqual(["a", "e"]);
  });

  it("autoCategorize never assigns the dedicated slots", () => {
    const r = autoCategorize(["text-embedding-3", "bge-reranker"]);
    expect(r.embedding).toBe("");
    expect(r.rerank).toBe("");
  });
});

describe("allConfiguredModelValues", () => {
  it("默认模型配置 ∪ 各时段档，展平去重保序", () => {
    const extra = JSON.stringify({ time_windows: [
      { windows: [], models: { default: "w1", opus: ["w2", "a"] } },
      { windows: [], models: { sonnet: "w3" } },
    ] });
    expect(allConfiguredModelValues({ models: { default: "a" }, extra })).toEqual(["a", "w1", "w2", "w3"]);
  });
  it("extra 缺省 / 非法 JSON → 只有默认配置", () => {
    expect(allConfiguredModelValues({ models: { default: "a" } })).toEqual(["a"]);
    expect(allConfiguredModelValues({ models: { default: "a" }, extra: "{bad" })).toEqual(["a"]);
  });
});

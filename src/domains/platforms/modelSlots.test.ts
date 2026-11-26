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

  it("image slot in MODEL_SLOTS with locale key", () => {
    const m = Object.fromEntries(MODEL_SLOTS.map(s => [s.key, s.labelKey]));
    expect(m.image).toBe("platform.modelImage");
  });

  it("DEDICATED_MODEL_SLOTS = embedding + rerank + image only", () => {
    expect([...DEDICATED_MODEL_SLOTS]).toEqual(["embedding", "rerank", "image"]);
  });

  it("allModelValues includes both slots, deduped", () => {
    expect(allModelValues({ default: "a", embedding: "e", rerank: "e" })).toEqual(["a", "e"]);
  });

  it("autoCategorize never assigns the dedicated slots", () => {
    const r = autoCategorize(["text-embedding-3", "bge-reranker", "gpt-image-2"]);
    expect(r.embedding).toBe("");
    expect(r.rerank).toBe("");
    expect(r.image).toBe("");
  });
});

describe("fable model slot", () => {
  it("categorizes Fable and includes it in configured models", () => {
    expect(MODEL_SLOTS.find(s => s.key === "fable")?.labelKey).toBe("platform.modelFable");
    expect(autoCategorize(["Claude-FABLE-5"]).fable).toBe("Claude-FABLE-5");
    expect(allModelValues({ fable: ["a", "b"] })).toEqual(["a", "b"]);
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
  it("关掉的时段档（enabled:false）视为不存在，与 Rust model_configs 对称", () => {
    const extra = JSON.stringify({ time_windows: [
      { windows: [], models: { default: "off" }, enabled: false },
      { windows: [], models: { default: "on" }, enabled: true },
    ] });
    expect(allConfiguredModelValues({ models: { default: "a" }, extra })).toEqual(["a", "on"]);
  });
  it("extra 缺省 / 非法 JSON → 只有默认配置", () => {
    expect(allConfiguredModelValues({ models: { default: "a" } })).toEqual(["a"]);
    expect(allConfiguredModelValues({ models: { default: "a" }, extra: "{bad" })).toEqual(["a"]);
  });
});

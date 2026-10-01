import { describe, expect, it } from "vitest";
import { safeParseJson } from "./types";

/**
 * O2（perf-backend spec §2）：proxy_log 上游正文改为存紧凑原文，展示侧格式化。
 * 两条展示路径（DetailPanel 经 safeParseJson + JSON.stringify(v, null, 2)、
 * useLogsDetail 复制用的 fj）依赖同一不变量：紧凑（新行）与 pretty（存量旧行）
 * 两种存库格式，经 parse → stringify(,2) 归一化后输出完全一致，且再 pretty 幂等。
 */
const pretty = (s: string) => {
  const v = safeParseJson(s);
  return typeof v === "string" ? v : JSON.stringify(v, null, 2);
};

const doc = {
  model: "claude-v4-5",
  messages: [{ role: "user", content: '你好 world é "quoted"' }],
  metadata: { nested: { deep: [1, 2, { three: 3.5 }] }, flag: true, none: null },
};

describe("logs detail body normalization (O2)", () => {
  const compact = JSON.stringify(doc);
  const storedPretty = JSON.stringify(doc, null, 2);

  it("compact (new rows) and pretty (legacy rows) render identically", () => {
    expect(pretty(compact)).toBe(pretty(storedPretty));
    expect(pretty(compact)).toBe(storedPretty);
  });

  it("pretty-ing an already-pretty value is idempotent", () => {
    expect(pretty(storedPretty)).toBe(storedPretty);
  });

  it("non-JSON body falls back to raw string", () => {
    expect(pretty("data: [DONE]\n\n")).toBe("data: [DONE]\n\n");
  });
});

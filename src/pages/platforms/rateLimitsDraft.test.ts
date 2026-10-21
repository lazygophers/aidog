// 限频平铺后的 draft 助手（原 RateLimitsEditModal 逻辑）：往返与净化。
import { describe, expect, it } from "vitest";
import { bundleToDraft, draftToBundle, type RateLimitsDraft } from "./formSections";
import type { RateLimitsBundle } from "../../services/api";

const D = (over: Partial<RateLimitsDraft> = {}): RateLimitsDraft => ({
  rpm: "", tpm: "", models: [], windows: [], ...over,
});

describe("rateLimits draft helpers", () => {
  it("bundleToDraft → draftToBundle 往返保持", () => {
    const b: RateLimitsBundle = {
      rate_limits: { rpm: 60, tpm: 1000, models: { "glm-4": { rpm: 10 } } },
      quota_windows: [{ window: 18000, budget_tokens: 5000, budget_points: undefined }],
    };
    expect(draftToBundle(bundleToDraft(b))).toEqual(b);
  });

  it("净化：非法数字丢弃、空模型名跳过、tokens 优先于 points", () => {
    const b = draftToBundle(D({
      rpm: "abc",
      tpm: "1.9",
      models: [{ model: "  ", rpm: "5", tpm: "" }, { model: "m1", rpm: "-3", tpm: "7" }],
      windows: [{ window: "0", budgetTokens: "9", budgetPoints: "" }, { window: "604800", budgetTokens: "8", budgetPoints: "9" }],
    }));
    expect(b.rate_limits).toEqual({ rpm: undefined, tpm: 1, models: { m1: { rpm: undefined, tpm: 7 } } });
    expect(b.quota_windows).toEqual([{ window: 604800, budget_tokens: 8, budget_points: undefined }]);
  });

  it("全空 → 两子键均 undefined", () => {
    expect(draftToBundle(D())).toEqual({ rate_limits: undefined, quota_windows: undefined });
  });
});

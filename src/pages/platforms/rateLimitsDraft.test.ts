// 限频平铺后的 draft 助手（原 RateLimitsEditModal 逻辑）：往返与净化。
// 2026-10-23 A2 定稿改形状：窗口 = num+unit（秒因子），预算三选一 tokens|usd|count。
import { describe, expect, it } from "vitest";
import { bundleToDraft, draftToBundle, type RateLimitsDraft } from "./formSections";
import type { RateLimitsBundle } from "../../services/api";

const D = (over: Partial<RateLimitsDraft> = {}): RateLimitsDraft => ({
  rpm: "", tpm: "", models: [], windows: [], ...over,
});

describe("rateLimits draft helpers", () => {
  it("bundleToDraft → draftToBundle 往返保持（tokens/usd/count 三档）", () => {
    const b: RateLimitsBundle = {
      rate_limits: { rpm: 60, tpm: 1000, models: { "glm-4": { rpm: 10 } } },
      quota_windows: [
        { window: 18000, budget_tokens: 5000 },
        { window: 604800, budget_usd: 12.5 },
        { window: 86400, budget_count: 200 },
      ],
    };
    expect(draftToBundle(bundleToDraft(b))).toEqual(b);
  });

  it("bundleToDraft 秒数拆 数字+单位：5h→(5,3600)、7d→(7,86400)、90min→(90,60)、45s→向上归分钟", () => {
    const d = bundleToDraft({ quota_windows: [{ window: 18000 }, { window: 604800 }, { window: 5400 }, { window: 45 }] });
    expect(d.windows.map(w => [w.num, w.unit])).toEqual([["5", "3600"], ["7", "86400"], ["90", "60"], ["1", "60"]]);
  });

  it("bundleToDraft：存量 points-only 行 → mode tokens、amount 空（编辑保存即清点数）", () => {
    const d = bundleToDraft({ quota_windows: [{ window: 18000, budget_points: 60 }] });
    expect(d.windows[0]).toMatchObject({ num: "5", unit: "3600", mode: "tokens", amount: "" });
    expect(draftToBundle(d).quota_windows).toBeUndefined();
  });

  it("净化：非法数字丢弃、空模型名跳过、空预算行整行丢", () => {
    const b = draftToBundle(D({
      rpm: "abc",
      tpm: "1.9",
      models: [{ model: "  ", rpm: "5", tpm: "" }, { model: "m1", rpm: "-3", tpm: "7" }],
      windows: [
        { num: "0", unit: "3600", mode: "tokens", amount: "9" },      // 时长非法 → 整行丢
        { num: "7", unit: "86400", mode: "tokens", amount: "" },      // 预算空 → 丢
        { num: "2", unit: "3600", mode: "usd", amount: "4.5" },       // 金额小数保留
        { num: "1", unit: "60", mode: "count", amount: "9.7" },       // 次数取整
      ],
    }));
    expect(b.rate_limits).toEqual({ rpm: undefined, tpm: 1, models: { m1: { rpm: undefined, tpm: 7 } } });
    expect(b.quota_windows).toEqual([{ window: 7200, budget_usd: 4.5 }, { window: 60, budget_count: 9 }]);
  });

  it("全空 → 两子键均 undefined", () => {
    expect(draftToBundle(D())).toEqual({ rate_limits: undefined, quota_windows: undefined });
  });
});

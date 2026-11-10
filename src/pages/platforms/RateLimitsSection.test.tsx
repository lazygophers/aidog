// RateLimitsSection：载入含 rules 的平台后保存不丢 rules；策略库导入后列表显示同一份 rules。
import { describe, it, expect, vi } from "vitest";
import { render, fireEvent } from "../../test/render";

vi.mock("../../domains/platforms/modelPrices", () => ({ useModelPrices: () => new Map() }));
vi.mock("../../domains/platforms", async (orig) => ({
  ...(await orig<typeof import("../../domains/platforms")>()),
  getDefaultRateLimitPolicies: vi.fn(async () => ({ policies: [], lastUpdated: 0 })),
}));
import { RateLimitsSection } from "./formSections";
import { rateLimitRuleError, type RateLimitsBundle } from "../../services/api";

const t = ((key: string, fallback?: string) => fallback ?? key) as never;
const RULES = [{ models: ["glm-4"], rpm: 10 }, { start_hour: 14, end_hour: 18, rpm: 5 }];

describe("RateLimitsSection rules", () => {
  it("改平台级 RPM 后 setBundle 带回原 rules", () => {
    const setBundle = vi.fn();
    const { container } = render(<RateLimitsSection bundle={{ rate_limits: { rpm: 5, rules: RULES } }} setBundle={setBundle} protocol={"glm" as never} modelOptions={[]} t={t} />);
    fireEvent.change(container.querySelector("#rl-rpm")!, { target: { value: "7" } });
    expect(setBundle.mock.calls.at(-1)![0].rate_limits).toEqual({ rpm: 7, rules: RULES });
  });

  it("外部 bundle 换成导入的 rules → 列表显示同一份", () => {
    const r = render(<RateLimitsSection bundle={{}} setBundle={vi.fn()} protocol={"glm" as never} modelOptions={[]} t={t} />);
    expect(r.container.querySelector('[data-testid="rl-rule-0"]')).toBeNull();
    const b: RateLimitsBundle = { rate_limits: { rules: RULES, source_policy_id: "v0" } };
    r.rerender(<RateLimitsSection bundle={b} setBundle={vi.fn()} protocol={"glm" as never} modelOptions={[]} t={t} />);
    expect((r.container.querySelector('[data-testid="rl-rule-1-start"]') as HTMLInputElement).value).toBe("14");
    expect(r.container.querySelector('[data-testid="rl-rule-0-chip-glm-4"]')).not.toBeNull();
  });
});

describe("rateLimitRuleError", () => {
  it("无条件 / 非法通配 / 合法", () => {
    expect(rateLimitRuleError({ rpm: 1 })).toBe("noCondition");
    expect(rateLimitRuleError({ models: [] , rpm: 1 })).toBe("noCondition");
    expect(rateLimitRuleError({ models: ["a*b"] })).toBe("badWildcard");
    expect(rateLimitRuleError({ models: ["*"] })).toBe("badWildcard");
    expect(rateLimitRuleError({ models: ["glm-*"] })).toBeNull();
    expect(rateLimitRuleError({ days_of_week: [1] })).toBeNull();
    expect(rateLimitRuleError({ start_hour: 0 })).toBeNull();
  });
});

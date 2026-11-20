// T3 DetailData 行为测试：渲染真组件、断言走 data-testid、invoke 按 command mock。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "../../../test/render";
import { DetailData } from "./DetailData";
import type { Platform } from "../../../services/api";

vi.mock("../../../services/transport", () => ({
  invoke: vi.fn(),
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import { invoke } from "../../../services/transport";
const mockedInvoke = vi.mocked(invoke);

// 固定 now：2026-11-20 12:00 UTC
const FIXED_NOW = Date.UTC(2026, 10, 20, 12, 0, 0);
vi.spyOn(Date, "now").mockReturnValue(FIXED_NOW);
const DAY = 86_400_000;
const dayKey = (offsetDays: number) => {
  const d = new Date(FIXED_NOW - offsetDays * DAY);
  const p = (n: number) => `${n}`.padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
};

const USAGE_STATS = {
  total_requests: 42, success_count: 40, total_input_tokens: 1000, total_output_tokens: 2000,
  total_cache_tokens: 300, cache_rate: 0, recent_failures: 0, recent_total: 0, total_cost: 1.25,
  today_tokens: 0, today_cost: 0, today_requests: 0, today_success: 0,
};
const SERIES = [
  { time: dayKey(1), requests: 4, success: 4, input_tokens: 400, output_tokens: 0, cache_tokens: 0, est_cost: 0.4, duration_ms: 100 },
  { time: dayKey(2), requests: 6, success: 6, input_tokens: 600, output_tokens: 0, cache_tokens: 0, est_cost: 0.6, duration_ms: 120 },
];
const HEALTH = {
  platform_id: 1, breaker_open: false, breaker_fails: 2, breaker_until_ms: 0,
  quota_cooldown_secs: 0, quota_verdict_secs: 0, auth_cooldown_secs: 300,
  balance_cooldown_secs: 0, last_connect_fail_ms: 0, peak_disabled: false, updated_at: FIXED_NOW,
};
const MODELS = [
  { model: "m1", requests: 60, tokens: 100, est_cost: 0.1 },
  { model: "m2", requests: 40, tokens: 50, est_cost: 0.2 },
];
const FAILS = {
  items: [
    { id: "1", group_key: "g", model: "m1", actual_model: "", source_protocol: "", target_protocol: "",
      platform_id: 1, status_code: 429, duration_ms: 0, input_tokens: 0, output_tokens: 0, cache_tokens: 0,
      blocked_reason: "", created_at: FIXED_NOW - 1000 } as never,
    { id: "2", group_key: "g", model: "m2", actual_model: "", source_protocol: "", target_protocol: "",
      platform_id: 1, status_code: 503, duration_ms: 0, input_tokens: 0, output_tokens: 0, cache_tokens: 0,
      blocked_reason: "", created_at: FIXED_NOW - 2000 } as never,
  ],
  has_more: false,
};
const QUOTA = {
  success: true, error: null, queried_at: FIXED_NOW,
  balance: null,
  coding_plan: {
    tiers: [
      { name: "weekly_limit", utilization: 0, resets_at: new Date(FIXED_NOW + 3 * 3600_000).toISOString(), limit: 2000, remaining: 2000 },
    ],
    level: null,
  },
};

beforeEach(() => {
  mockedInvoke.mockImplementation((cmd: string) => {
    switch (cmd) {
      case "platform_usage_stats": return Promise.resolve(USAGE_STATS);
      case "platform_usage_series": return Promise.resolve(SERIES);
      case "platform_health_detail": return Promise.resolve(HEALTH);
      case "platform_model_distribution": return Promise.resolve(MODELS);
      case "proxy_log_list_filtered": return Promise.resolve(FAILS);
      case "platform_query_quota": return Promise.resolve(QUOTA);
      default: return Promise.resolve(undefined);
    }
  });
});

const estPlanJson = (windowStart: number) => JSON.stringify({
  tiers: [{ name: "weekly_limit", est_utilization: 20, coef_per_token: 0, util_at_last_real: 0, tokens_since_real: 0, has_base: true, limit: 2000, window_start: windowStart }],
  level: null,
});

const basePlatform = (over: Partial<Platform>): Platform => ({
  id: 1, name: "P", platform_type: "anthropic", base_url: "https://api.anthropic.com",
  api_key: "sk-test", endpoints: [], models: { default: [] }, available_models: [],
  enabled: true, status: "enabled", auto_disabled_until: 0, auto_disable_strikes: 0,
  created_at: 0, updated_at: 0, deleted_at: 0, est_balance_remaining: 0,
  est_coding_plan: "", last_real_query_at: 0, estimate_count: 0,
  show_in_tray: false, tray_display: "balance", manual_budgets: [], expires_at: 0,
  ...over,
} as Platform);

describe("DetailData（T3 数据板块）", () => {
  it("订阅平台：渲染平滑拟合曲线，无按量柱图，发 quota 查询，偏差 >5% 有图注", async () => {
    render(<DetailData platform={basePlatform({ platform_type: "kimi_coding", est_coding_plan: estPlanJson(FIXED_NOW - 2 * DAY) })} />);
    expect(await screen.findByTestId("detail-t3-curve")).toBeInTheDocument();
    expect(screen.queryByTestId("detail-t3-paygo")).not.toBeInTheDocument();
    // quota 查询已发起（快照校准）
    const quotaCall = mockedInvoke.mock.calls.find(c => c[0] === "platform_query_quota");
    expect(quotaCall).toBeTruthy();
    // quota utilization 0 → 校准点 100%，本地 ~50% → 偏差 > 5%
    expect(await screen.findByTestId("detail-t3-deviation")).toBeInTheDocument();
    // 曲线要素：拟合线 + 校准点 SVG 在场
    expect(document.querySelector('[data-testid="detail-t3-curve"] path')).toBeTruthy();
    expect(document.querySelector('[data-testid="detail-t3-curve"] circle')).toBeTruthy();
  });

  it("按量平台：渲染 7 天柱线图 + 余额，无曲线与重置文案", async () => {
    render(<DetailData platform={basePlatform({ platform_type: "deepseek", est_balance_remaining: 5 })} />);
    expect(await screen.findByTestId("detail-t3-paygo")).toBeInTheDocument();
    expect(screen.queryByTestId("detail-t3-curve")).not.toBeInTheDocument();
    expect(screen.getByTestId("detail-t3-balance")).toHaveTextContent("$5.00");
    const quotaCall = mockedInvoke.mock.calls.find(c => c[0] === "platform_query_quota");
    expect(quotaCall).toBeUndefined();
  });

  it("统计板块：四档窗口切换带 window 参数重新拉 stats", async () => {
    render(<DetailData platform={basePlatform({})} />);
    await screen.findByTestId("detail-t3-stats");
    fireEvent.click(screen.getByTestId("detail-t3-win-30d"));
    await vi.waitFor(() => {
      const call = mockedInvoke.mock.calls.filter(c => c[0] === "platform_usage_stats").at(-1);
      expect(call?.[1]).toMatchObject({ platformId: 1, window: "30d" });
    });
    // 统计行渲染
    expect(screen.getAllByTestId("detail-t3-stat-row").length).toBeGreaterThan(0);
  });

  it("健康板块：渲染熔断/冷却/高峰禁用（数据来自 platform_health_detail）", async () => {
    render(<DetailData platform={basePlatform({})} />);
    expect(await screen.findByTestId("detail-t3-breaker")).toBeInTheDocument();
    // auth_cooldown_secs=300 → 冷却行非「无」
    expect(document.querySelectorAll('[data-testid="detail-t3-cd-row"]').length).toBeGreaterThanOrEqual(4);
  });

  it("失败板块：分类条形 + 最近失败表（429→限频、503→服务类）", async () => {
    render(<DetailData platform={basePlatform({})} />);
    await screen.findByTestId("detail-t3-fail");
    // 测试 i18n 空 resources：缺 key 回传 key 本身，断言 key 而非译文
    expect(screen.getAllByText("platform.detail.failRate").length).toBeGreaterThan(0);
    expect(screen.getAllByText("platform.detail.failService").length).toBeGreaterThan(0);
    // 最近失败表含状态码 429 行
    expect(screen.getByText("429")).toBeInTheDocument();
    expect(screen.getByText("503")).toBeInTheDocument();
  });

  it("模型分布：按请求数降序渲染行 + 占比", async () => {
    render(<DetailData platform={basePlatform({})} />);
    const rows = await screen.findAllByTestId("detail-t3-model-row");
    expect(rows.length).toBe(2);
    expect(rows[0]).toHaveTextContent("m1");
    expect(rows[0]).toHaveTextContent("60%");
  });
});

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
const FIXED_NOW = Date.UTC(2025, 1, 19, 12, 0, 0);
vi.spyOn(Date, "now").mockReturnValue(FIXED_NOW);
const DAY = 86_400_000;
const CURVE_WINDOW_START = new Date(FIXED_NOW - 6 * DAY);
CURVE_WINDOW_START.setHours(0, 0, 0, 0);
const CURVE_RESET_AT = CURVE_WINDOW_START.getTime() + 7 * DAY;
const dayKey = (offsetDays: number) => {
  const d = new Date(FIXED_NOW - offsetDays * DAY);
  const p = (n: number) => `${n}`.padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
};
const QUOTA_QUERY_AT = new Date(`${dayKey(1)}T00:00:00`);
QUOTA_QUERY_AT.setDate(QUOTA_QUERY_AT.getDate() + 1);

const USAGE_STATS = {
  total_requests: 42, success_count: 40, total_input_tokens: 1000, total_output_tokens: 2000,
  total_cache_tokens: 300, cache_rate: 0, recent_failures: 0, recent_total: 0, total_cost: 1.25,
  today_tokens: 0, today_cost: 0, today_requests: 0, today_success: 0,
};
const SERIES = [
  { time: dayKey(3), requests: 3, success: 3, input_tokens: 300, output_tokens: 0, cache_tokens: 0, est_cost: 0.3, duration_ms: 110 },
  { time: dayKey(2), requests: 4, success: 4, input_tokens: 400, output_tokens: 0, cache_tokens: 0, est_cost: 0.4, duration_ms: 100 },
  { time: dayKey(1), requests: 6, success: 6, input_tokens: 600, output_tokens: 0, cache_tokens: 0, est_cost: 0.6, duration_ms: 120 },
  { time: dayKey(0), requests: 1, success: 1, input_tokens: 100, output_tokens: 0, cache_tokens: 0, est_cost: 0.1, duration_ms: 100 },
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
const QUOTA_LEVELS = [
  { key: "3600|count|2", dimension: "count", reset_mode: "sliding", used: 1, budget: 2, remaining_ratio: 0.5, resets_at_ms: FIXED_NOW + 3_600_000 },
  { key: "18000|tokens|120000", dimension: "tokens", reset_mode: "calendar_day", used: 40_000, budget: 120_000, remaining_ratio: 2 / 3, resets_at_ms: FIXED_NOW + 7_200_000 },
];
// 限频实时占用：平台级桶（空 model）+ 模型级桶，全部真实计数（后端 SchedulerState.rate 快照）。
const RATE_OCC = [
  { model: "", in_flight: 2, rpm_used: 7, tpm_used: 12_300 },
  { model: "m1", in_flight: 1, rpm_used: 3, tpm_used: 800 },
];
const QUOTA = {
  success: true, error: null, queried_at: QUOTA_QUERY_AT.getTime(),
  balance: null,
  coding_plan: {
    tiers: [
      { name: "weekly_limit", unit: "tokens", utilization: 0, resets_at: new Date(CURVE_RESET_AT).toISOString(), limit: 2000, remaining: 2000 },
    ],
    level: null,
  },
};

const baseMock = (cmd: string) => {
  switch (cmd) {
    case "platform_usage_stats": return Promise.resolve(USAGE_STATS);
    case "platform_usage_series": return Promise.resolve(SERIES);
    case "platform_health_detail": return Promise.resolve(HEALTH);
    case "platform_model_distribution": return Promise.resolve(MODELS);
    case "proxy_log_list_filtered": return Promise.resolve(FAILS);
    case "platform_query_quota": return Promise.resolve(QUOTA);
    case "platform_quota_window_levels": return Promise.resolve(QUOTA_LEVELS);
    case "platform_rate_occupancy": return Promise.resolve(RATE_OCC);
    default: return Promise.resolve(undefined);
  }
};

beforeEach(() => {
  mockedInvoke.mockImplementation(baseMock);
});

const estPlanJson = (windowStart: number) => JSON.stringify({
  tiers: [{ name: "weekly_limit", unit: "tokens", est_utilization: 20, coef_per_token: 0, util_at_last_real: 0, tokens_since_real: 0, has_base: true, limit: 2000, window_start: windowStart }],
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
    render(<DetailData platform={basePlatform({ platform_type: "kimi_coding", est_coding_plan: estPlanJson(FIXED_NOW - 4 * DAY) })} />);
    expect(await screen.findByTestId("detail-t3-curve")).toBeInTheDocument();
    expect(screen.queryByTestId("detail-t3-paygo")).not.toBeInTheDocument();
    // quota 查询已发起（快照校准）
    const quotaCall = mockedInvoke.mock.calls.find(c => c[0] === "platform_query_quota");
    expect(quotaCall).toBeTruthy();
    // quota utilization 0 → 校准点 100%；最后小时桶与快照同刻，可比较偏差。
    expect(await screen.findByTestId("detail-t3-deviation")).toBeInTheDocument();
    // 曲线要素：拟合线 + 校准点 SVG 在场
    expect(screen.getByTestId("detail-t3-fit")).toBeInTheDocument();
    expect(document.querySelector('[data-testid="detail-t3-curve"] circle')).toBeTruthy();
  });

  it("少于三点时只画原始采样，不拟合曲线并说明数据不足", async () => {
    mockedInvoke.mockImplementation((cmd: string) =>
      cmd === "platform_usage_series" ? Promise.resolve(SERIES.slice(0, 1)) : baseMock(cmd));
    render(<DetailData platform={basePlatform({ platform_type: "kimi_coding", est_coding_plan: estPlanJson(FIXED_NOW - 4 * DAY) })} />);
    expect(await screen.findByTestId("detail-t3-sparse")).toHaveTextContent("platform.detail.curveSparse");
    expect(screen.queryByTestId("detail-t3-fit")).not.toBeInTheDocument();
    expect(screen.getAllByTestId("detail-t3-raw-point")).toHaveLength(1);
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
    // 40 / 42 = 95.2%；formatter 接受 0–100 百分数。
    expect(screen.getAllByTestId("detail-t3-stat-row")[1]).toHaveTextContent("95.2%");
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

  it("水位板块：额度窗口逐条渲染 used/budget 与剩余比例（disabled 窗口由后端过滤）", async () => {
    render(<DetailData platform={basePlatform({})} />);
    await screen.findByTestId("detail-t5-water");
    const rows = await screen.findAllByTestId("detail-t5-qw-row");
    expect(rows.length).toBe(2);
    // 1/2 已用 → 值「1 / 2」；维度/重置机制 key 映射（测试 i18n 空资源 → key 原文）
    expect(rows[0]).toHaveTextContent("1 / 2");
    expect(rows[0]).toHaveTextContent("platform.detail.qwCount");
    expect(rows[0]).toHaveTextContent("platform.detail.modeSliding");
    // token 窗口 40K/120K
    expect(rows[1]).toHaveTextContent("40.0K / 120.0K");
    expect(rows[1]).toHaveTextContent("platform.detail.qwTokens");
    expect(rows[1]).toHaveTextContent("platform.detail.modeCalendarDay");
    // 已用比例条：50% / 33.3%（fill 宽度 = used/budget）
    expect(screen.getByTestId("detail-t5-qw-fill-0").style.width).toBe("50%");
    expect(screen.getByTestId("detail-t5-qw-fill-1").style.width).toBe("33.3%");
    // 命令已发起
    const call = mockedInvoke.mock.calls.find(c => c[0] === "platform_quota_window_levels");
    expect(call?.[1]).toMatchObject({ platformId: 1 });
  });

  it("水位板块：无窗口配置 → 空态", async () => {
    mockedInvoke.mockImplementation((cmd: string) =>
      cmd === "platform_quota_window_levels" ? Promise.resolve([]) : baseMock(cmd));
    render(<DetailData platform={basePlatform({})} />);
    await screen.findByTestId("detail-t5-water");
    expect(screen.queryByTestId("detail-t5-qw-row")).not.toBeInTheDocument();
  });

  it("水位板块：限频占用行渲染真实计数（平台级含配置上限，模型级只显示计数）", async () => {
    render(<DetailData platform={basePlatform({
      extra: JSON.stringify({ rate_limits: { concurrency: 5, rpm: 60, tpm: 100000 } }),
    })} />);
    await screen.findByTestId("detail-t5-water");
    const rows = await screen.findAllByTestId("detail-t5-occ-row");
    expect(rows.length).toBe(2);
    // 平台级（空 model → 平台级标签）+ 三计数原值
    expect(rows[0]).toHaveTextContent("platform.detail.rlPlatformLevel");
    expect(rows[0]).toHaveTextContent("2 / 7 / 12.3K");
    // 平台级桶显示 extra.rate_limits 配置上限
    expect(rows[0]).toHaveTextContent("platform.detail.rlConfigured");
    // 模型级桶显示模型名，无配置上限行（模型级上限依赖规则引擎，前端不复算）
    expect(rows[1]).toHaveTextContent("m1");
    expect(rows[1]).toHaveTextContent("1 / 3 / 800");
    expect(rows[1].textContent).not.toContain("platform.detail.rlConfigured");
    // 命令已发起
    const call = mockedInvoke.mock.calls.find(c => c[0] === "platform_rate_occupancy");
    expect(call?.[1]).toMatchObject({ platformId: 1 });
  });

  it("水位板块：限频无占用（代理未启动 → 空列表）→ 不渲染占用行，额度窗口照常", async () => {
    mockedInvoke.mockImplementation((cmd: string) =>
      cmd === "platform_rate_occupancy" ? Promise.resolve([]) : baseMock(cmd));
    render(<DetailData platform={basePlatform({})} />);
    await screen.findByTestId("detail-t5-water");
    expect(screen.queryByTestId("detail-t5-occ-row")).not.toBeInTheDocument();
    expect((await screen.findAllByTestId("detail-t5-qw-row")).length).toBe(2);
  });

  it("水位板块：限频占用事件驱动——订阅 rate-occupancy-updated，事件触发（1s debounce）重拉", async () => {
    const { listen } = await import("../../../services/transport");
    const handlers = new Map<string, (e: { payload: unknown }) => void>();
    vi.mocked(listen).mockImplementation((async (event: string, handler: (e: { payload: unknown }) => void) => {
      handlers.set(event, handler);
      return () => {};
    }) as never);
    render(<DetailData platform={basePlatform({})} />);
    await screen.findByTestId("detail-t5-water");
    // 前端已订阅限频占用事件（不再 setInterval 轮询）
    expect(handlers.has("rate-occupancy-updated")).toBe(true);
    const countCalls = () =>
      mockedInvoke.mock.calls.filter(c => c[0] === "platform_rate_occupancy").length;
    const before = countCalls();
    vi.useFakeTimers();
    try {
      handlers.get("rate-occupancy-updated")!({ payload: null });
      await vi.advanceTimersByTimeAsync(1100); // debounce 1s 后重拉
      expect(countCalls()).toBeGreaterThan(before);
    } finally {
      vi.useRealTimers();
    }
  });
});

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render } from "../../test/render";
import { PlatformCard } from "./PlatformCard";
import type { Platform, PlatformQuota, PlatformUsageStats, LastTestResult } from "../../services/api";

// Mock Tauri invoke
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// Mock 异步 domains 函数
vi.mock("../../domains/platforms", async () => {
  const actual = await vi.importActual("../../domains/platforms");
  return {
    ...actual,
    getDefaultModels: vi.fn().mockResolvedValue({ "claude-3-5-sonnet": "claude-3-5-sonnet" }),
    getDefaultPeakHours: vi.fn().mockResolvedValue([]),
    getProtocolHomepage: vi.fn().mockResolvedValue(""),
    isCodingPlanProtocol: vi.fn().mockResolvedValue(false),
  };
});

vi.mock("../../domains/platforms/defaults", () => ({
  getProtocolLabel: vi.fn().mockResolvedValue("Anthropic"),
  getProtocolLabelMap: vi.fn().mockResolvedValue({}),
  getProtocolColorMap: vi.fn().mockResolvedValue({}),
  getDefaultPeakHours: vi.fn().mockResolvedValue([]),
}));

vi.mock("../../domains/platforms/useProtocolLogo", () => ({
  useProtocolLogo: vi.fn().mockReturnValue({ logoSrc: null }),
}));

// 固定 Date.now 避免时间相关快照波动
const FIXED_NOW = 1_740_000_000_000;
vi.spyOn(Date, "now").mockReturnValue(FIXED_NOW);

describe("PlatformCard", () => {
  const mockActions = {
    onPointerDown: vi.fn(),
    onPointerMove: vi.fn(),
    onPointerUp: vi.fn(),
    onToggleExpanded: vi.fn(),
    onRefreshQuota: vi.fn(),
    onToggleEnabled: vi.fn(),
    onEdit: vi.fn(),
    onShare: vi.fn(),
    onDuplicate: vi.fn(),
    onDelete: vi.fn(),
    onViewLogs: vi.fn(),
    onQuickTest: vi.fn(),
    onCustomTest: vi.fn(),
    onFaviconFailed: vi.fn(),
  };

  const basePlatform: Platform = {
    id: 1,
    name: "Test Platform",
    platform_type: "anthropic",
    base_url: "https://api.anthropic.com",
    api_key: "sk-test",
    extra: "",
    models: {},
    available_models: [],
    endpoints: [],
    enabled: true,
    status: "enabled",
    auto_disabled_until: 0,
    auto_disable_strikes: 0,
    created_at: FIXED_NOW - 100_000,
    updated_at: FIXED_NOW - 50_000,
    deleted_at: 0,
    est_balance_remaining: 0,
    est_coding_plan: "",
    last_real_query_at: 0,
    estimate_count: 0,
    show_in_tray: false,
    tray_display: "balance",
    manual_budgets: [],
    expires_at: 0,
  };

  const baseProps = {
    platform: basePlatform,
    index: 0,
    isDragging: false,
    dragActive: false,
    quotaRaw: undefined,
    quotaPreferReal: false,
    refreshing: false,
    usage: undefined,
    expanded: false,
    manualResult: undefined,
    testing: false,
    faviconFailed: false,
    actions: mockActions,
    draggable: true,
  };

  it("折叠态: 默认 platform 渲染", () => {
    const { container } = render(<PlatformCard {...baseProps} />);
    expect(container.firstChild).toMatchSnapshot();
  });

  it("展开态: expanded=true 显示明细", () => {
    const props = {
      ...baseProps,
      expanded: true,
      usage: {
        total_requests: 100,
        success_count: 95,
        total_input_tokens: 10000,
        total_output_tokens: 5000,
        total_cost: 1.5,
        today_tokens: 1000,
        today_cost: 0.15,
        recent_total: 10,
        recent_failures: 1,
      } as PlatformUsageStats,
    };
    const { container } = render(<PlatformCard {...props} />);
    expect(container.firstChild).toMatchSnapshot();
  });

  it("健康态: auto_disabled + last_error 显示警告徽标", () => {
    const props = {
      ...baseProps,
      platform: {
        ...basePlatform,
        status: "auto_disabled" as const,
        last_error: "HTTP 401",
        last_error_at: FIXED_NOW - 10_000,
        auto_disabled_until: FIXED_NOW + 100_000,
      },
    };
    const { container } = render(<PlatformCard {...props} />);
    expect(container.firstChild).toMatchSnapshot();
  });

  it("余额态: quotaRaw 显示余额条", () => {
    const props = {
      ...baseProps,
      quotaRaw: {
        balanceRemaining: 15.5,
        balanceTotal: 100,
        currency: "USD",
        tiers: [],
        hasData: true,
      } as PlatformQuota,
    };
    const { container } = render(<PlatformCard {...props} />);
    expect(container.firstChild).toMatchSnapshot();
  });

  it("测试态: testing=true 禁用测试按钮", () => {
    const props = {
      ...baseProps,
      testing: true,
    };
    const { container } = render(<PlatformCard {...props} />);
    expect(container.firstChild).toMatchSnapshot();
  });

  it("高峰态: lastTest 显示测试徽标", () => {
    const props = {
      ...baseProps,
      lastTest: {
        created_at: FIXED_NOW - 5000,
        duration_ms: 1234,
        success: true,
        error: "",
        response_body: "",
      } as LastTestResult,
    };
    const { container } = render(<PlatformCard {...props} />);
    expect(container.firstChild).toMatchSnapshot();
  });
});

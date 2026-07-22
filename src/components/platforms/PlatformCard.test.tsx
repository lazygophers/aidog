// ponytail: 改测行为（文本/角色/交互断言），禁快照/className 断言。
// 迁 shadcn 后 className 随样式变，快照脆；改测「渲染了什么 + 点击派发什么」。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "../../test/render";
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

// 固定 Date.now 避免时间相关断言波动
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

  beforeEach(() => {
    Object.values(mockActions).forEach(fn => fn.mockClear());
  });

  it("折叠态: 渲染平台名 + 操作按钮", () => {
    render(<PlatformCard {...baseProps} />);
    expect(screen.getByText("Test Platform")).toBeInTheDocument();
    // 操作按钮（按 title = i18n key 查询，不依赖图标/className）
    expect(screen.getByTitle("platform.viewLogs")).toBeInTheDocument();
    expect(screen.getByTitle("platform.share.button")).toBeInTheDocument();
    expect(screen.getByTitle("platform.duplicate")).toBeInTheDocument();
    // 折叠态无明细区（无 usage → hasDetail=false，不渲染 toggle 展开控件）
    expect(screen.queryByRole("button", { name: "platform.toggleDetail" })).not.toBeInTheDocument();
  });

  it("操作按钮点击: 派发对应 handler（携带 platform）", () => {
    render(<PlatformCard {...baseProps} />);
    fireEvent.click(screen.getByTitle("platform.viewLogs"));
    expect(mockActions.onViewLogs).toHaveBeenCalledWith(basePlatform);
    fireEvent.click(screen.getByTitle("platform.share.button"));
    expect(mockActions.onShare).toHaveBeenCalledWith(basePlatform);
    fireEvent.click(screen.getByTitle("platform.duplicate"));
    expect(mockActions.onDuplicate).toHaveBeenCalledWith(basePlatform);
  });

  it("启用/禁用开关: 点击派发 onToggleEnabled", () => {
    render(<PlatformCard {...baseProps} />);
    // toggle 是 div（非 button），按 title 查（status=enabled → title="platform.disable"）
    const toggle = screen.getByTitle("platform.disable");
    fireEvent.click(toggle);
    expect(mockActions.onToggleEnabled).toHaveBeenCalledWith(basePlatform);
  });

  it("展开态: usage 有数据时显示明细 toggle + 点击派发 onToggleExpanded", () => {
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
    render(<PlatformCard {...props} />);
    // toggle 走 aria-label；空 resources i18n 回退返回 key（render.tsx 约定）
    const toggle = screen.getByRole("button", { name: "platform.toggleDetail" });
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    fireEvent.click(toggle);
    expect(mockActions.onToggleExpanded).toHaveBeenCalledWith(1, false);
  });

  it("健康态: auto_disabled 显示自动禁用徽标", () => {
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
    render(<PlatformCard {...props} />);
    expect(screen.getByText("platform.autoDisabled")).toBeInTheDocument();
  });

  it("余额态: quotaRaw 渲染刷新额度按钮 + 点击派发 onRefreshQuota", () => {
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
    render(<PlatformCard {...props} />);
    const refreshBtn = screen.getByTitle("platform.quotaRefresh");
    expect(refreshBtn).toBeInTheDocument();
    fireEvent.click(refreshBtn);
    expect(mockActions.onRefreshQuota).toHaveBeenCalledWith(props.platform);
  });

  it("测试态: testing=true 禁用快速测试按钮", () => {
    const props = { ...baseProps, testing: true };
    render(<PlatformCard {...props} />);
    const quickBtn = screen.getByTitle("platform.quickTest");
    expect(quickBtn).toBeDisabled();
    fireEvent.click(quickBtn);
    expect(mockActions.onQuickTest).not.toHaveBeenCalled();
  });

  it("快速测试/自定义测试按钮: 非 testing 态点击派发 handler", () => {
    render(<PlatformCard {...baseProps} />);
    fireEvent.click(screen.getByTitle("platform.quickTest"));
    expect(mockActions.onQuickTest).toHaveBeenCalledWith(basePlatform);
    fireEvent.click(screen.getByTitle("platform.customTest"));
    expect(mockActions.onCustomTest).toHaveBeenCalledWith(basePlatform);
  });

  it("高峰态: lastTest success 渲染 ✓ 徽标 + 时长", () => {
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
    render(<PlatformCard {...props} />);
    expect(screen.getByText("✓")).toBeInTheDocument();
    expect(screen.getByText("1234ms")).toBeInTheDocument();
  });
});

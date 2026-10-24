// ponytail: 改测行为（文本/角色/交互断言），禁快照/className 断言。
// 迁 shadcn 后 className 随样式变，快照脆；改测「渲染了什么 + 点击派发什么」。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "../../test/render";
import userEvent from "@testing-library/user-event";
import { PlatformCard } from "./PlatformCard";
import type { Platform, PlatformQuota, PlatformUsageStats, LastTestResult } from "../../services/api";
import { platformHasQuotaScript } from "../../domains/platforms/defaults";
import { useProtocolMeta } from "../../domains/platforms/useProtocolMeta";

// Mock Tauri invoke
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// Mock 异步 domains 函数（仍由 health.ts / 其他模块 import 间接触发）
vi.mock("../../domains/platforms", async () => {
  const actual = await vi.importActual("../../domains/platforms");
  return {
    ...actual,
  };
});

vi.mock("../../domains/platforms/defaults", () => ({
  getDefaultModels: vi.fn().mockResolvedValue({ "claude-3-5-sonnet": "claude-3-5-sonnet" }),
  getDefaultPeak: vi.fn().mockResolvedValue([]),
  getProtocolHomepage: vi.fn().mockResolvedValue(""),
  getProtocolSourceUrls: vi.fn().mockResolvedValue({ docs: "", pricing: "" }),
  isCodingPlanProtocol: vi.fn().mockResolvedValue(false),
  getProtocolLabel: vi.fn().mockResolvedValue("Anthropic"),
  getProtocolLabelMap: vi.fn().mockResolvedValue({}),
  getProtocolColorMap: vi.fn().mockResolvedValue({}),
  // quota_scripts 同步派生（T6）：默认可查（测试卡多数断言余额区）
  platformHasQuotaScript: vi.fn().mockReturnValue(true),
}));

vi.mock("../../domains/platforms/useProtocolLogo", () => ({
  useProtocolLogo: vi.fn().mockReturnValue({ logoSrc: null }),
}));

// useProtocolMeta 聚合 5 派生：单 hook 调用替代 5 effect，mock 直返同步值
vi.mock("../../domains/platforms/useProtocolMeta", () => ({
  useProtocolMeta: vi.fn().mockReturnValue({
    color: "var(--accent)",
    isCpProtocol: false,
    defaultModels: [],
    homepage: "",
    sourceUrls: { docs: "", pricing: "" },
    protocolLabel: "Anthropic",
    labelMap: {},
    peakWindows: [],
  }),
}));

// getPrimaryBaseUrl 从 usePlatformQuota 导入；mock 避免依赖真实 Platform 结构
vi.mock("../../pages/platforms/usePlatformQuota", async () => {
  const actual = await vi.importActual("../../pages/platforms/usePlatformQuota").catch(() => ({}));
  return {
    ...actual,
    getPrimaryBaseUrl: vi.fn((proto: string, eps: { base_url?: string }[] | undefined) =>
      eps?.[0]?.base_url ?? ""),
  };
});

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
    onDelete: vi.fn(),
    onFaviconFailed: vi.fn(),
  };
  // 票 12：只吃 platform 的 7 个回调拆进 service 对象
  const mockService = {
    onToggleEnabled: vi.fn(),
    onEdit: vi.fn(),
    onShare: vi.fn(),
    onDuplicate: vi.fn(),
    onViewLogs: vi.fn(),
    onQuickTest: vi.fn(),
    onCustomTest: vi.fn(),
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
    service: mockService,
    draggable: true,
  };

  beforeEach(() => {
    Object.values(mockActions).forEach(fn => fn.mockClear());
    Object.values(mockService).forEach(fn => fn.mockClear());
  });

  it("折叠态: 渲染平台名 + 行内操作 + ⋯ 菜单触发器", () => {
    render(<PlatformCard {...baseProps} />);
    expect(screen.getByText("Test Platform")).toBeInTheDocument();
    // 行内操作（title = i18n key）：开关 + 测速对 + 编辑 + ⋯ 触发器
    expect(screen.getByTitle("platform.disable")).toBeInTheDocument();
    expect(screen.getByTitle("platform.quickTest")).toBeInTheDocument();
    expect(screen.getByTitle("platform.customTest")).toBeInTheDocument();
    expect(screen.getByTitle("platform.editHint")).toBeInTheDocument();
    expect(screen.getByTitle("platform.moreActions")).toBeInTheDocument();
    // 低频操作收进 ⋯ 菜单：触发器不在场即菜单未开，菜单项不直接渲染
    expect(screen.queryByTitle("platform.viewLogs")).not.toBeInTheDocument();
    // 折叠态无明细区（无 usage → hasDetail=false，不渲染 toggle 展开控件）
    expect(screen.queryByRole("button", { name: "platform.toggleDetail" })).not.toBeInTheDocument();
  });

  it("⋯ 菜单: 打开后 日志/分享/复制/删除 可点且派发 handler（携带 platform）", async () => {
    const user = userEvent.setup();
    render(<PlatformCard {...baseProps} />);
    await user.click(screen.getByTitle("platform.moreActions"));
    expect(screen.getByText("platform.viewLogs")).toBeInTheDocument();
    await user.click(screen.getByText("platform.viewLogs"));
    expect(mockService.onViewLogs).toHaveBeenCalledWith(basePlatform);
    await user.click(screen.getByTitle("platform.moreActions"));
    await user.click(screen.getByText("platform.share.button"));
    expect(mockService.onShare).toHaveBeenCalledWith(basePlatform);
    await user.click(screen.getByTitle("platform.moreActions"));
    await user.click(screen.getByText("platform.duplicate"));
    expect(mockService.onDuplicate).toHaveBeenCalledWith(basePlatform);
    await user.click(screen.getByTitle("platform.moreActions"));
    await user.click(screen.getByText("platform.delete"));
    expect(mockActions.onDelete).toHaveBeenCalledWith(basePlatform.id);
  });

  it("启用/禁用开关: 点击派发 onToggleEnabled", () => {
    render(<PlatformCard {...baseProps} />);
    // toggle 是 div（非 button），按 title 查（status=enabled → title="platform.disable"）
    const toggle = screen.getByTitle("platform.disable");
    fireEvent.click(toggle);
    expect(mockService.onToggleEnabled).toHaveBeenCalledWith(basePlatform);
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
    expect(mockService.onQuickTest).not.toHaveBeenCalled();
  });

  it("快速测试/自定义测试按钮: 非 testing 态点击派发 handler", () => {
    render(<PlatformCard {...baseProps} />);
    fireEvent.click(screen.getByTitle("platform.quickTest"));
    expect(mockService.onQuickTest).toHaveBeenCalledWith(basePlatform);
    fireEvent.click(screen.getByTitle("platform.customTest"));
    expect(mockService.onCustomTest).toHaveBeenCalledWith(basePlatform);
  });

  it("余额行门控: 平台不支持配额查询，coding 已用 tokens 仍然显示", () => {
    // 回归 2026-09-22：原先整行锁在 `showQuota = quotaCapable && quota.hasData` 之后，
    // 于是「coding 已用 tokens」这块明明有数据也被一起挡住。它与配额是两个维度。
    vi.mocked(platformHasQuotaScript).mockReturnValueOnce(false);
    render(
      <PlatformCard
        {...baseProps}
        platform={{
          ...basePlatform,
          endpoints: [{ protocol: "anthropic", base_url: "https://x", coding_plan: true }],
        } as Platform}
        usage={{
          total_requests: 10,
          success_count: 10,
          total_input_tokens: 7000,
          total_output_tokens: 3000,
          total_cost: 2,
          today_tokens: 100,
          today_cost: 0.1,
          recent_total: 1,
          recent_failures: 0,
        } as PlatformUsageStats}
      />,
    );
    // 10000 tokens 经 formatNumber → "10K"（与 React/Flutter 同一份格式化口径）
    expect(screen.getByTitle("platform.codingUsedHint")).toBeInTheDocument();
  });

  it("高峰徽标: 时段来自 preset 默认（用户没自配也要显）", () => {
    // 回归 2026-09-22：原先只读 `parsePlatformPeak(p.extra)`，不回落 preset，
    // glm_coding / deepseek 这类自带预设高峰的平台在窗口内也不显徽标。
    // start_hour == end_hour → 退化为全天命中，断言不随运行时刻波动。
    vi.mocked(useProtocolMeta).mockReturnValueOnce({
      color: "var(--accent)",
      isCpProtocol: false,
      defaultModels: [],
      homepage: "",
      sourceUrls: { docs: "", pricing: "" },
      protocolLabel: "Anthropic",
      labelMap: {},
      peakWindows: [{ start_hour: 0, end_hour: 0, multiplier: 2 }],
    });
    render(<PlatformCard {...baseProps} />);
    expect(screen.getByText("platform.peak_badge")).toBeInTheDocument();
  });

  it("订阅徽标: claude_code 协议显示、anthropic 不显示（spec §3.5）", () => {
    const { rerender } = render(<PlatformCard {...baseProps} platform={{ ...basePlatform, platform_type: "claude_code" }} />);
    expect(screen.getByText("platform.subscriptionBadge")).toBeInTheDocument();
    // 非订阅协议不带徽标
    rerender(<PlatformCard {...baseProps} />);
    expect(screen.queryByText("platform.subscriptionBadge")).not.toBeInTheDocument();
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

  // 票 12 验收：memo 渲染策略不回退。渲染计数以 useProtocolMeta 调用次数代理
  // （每次 PlatformCard 函数体执行恰好调一次，且已被 mock）。
  it("memo: 相同 props rerender 不重渲、props 变化正常重渲（渲染计数对比）", () => {
    const metaSpy = vi.mocked(useProtocolMeta);
    metaSpy.mockClear();
    const { rerender } = render(<PlatformCard {...baseProps} />);
    expect(metaSpy).toHaveBeenCalledTimes(1);
    // 同一 props（含同一 actions/service 对象引用）再 rerender → memo 浅比较全等，不重渲
    rerender(<PlatformCard {...baseProps} />);
    expect(metaSpy).toHaveBeenCalledTimes(1);
    // props 实际变化（expanded）→ 正常重渲，证明 memo 没有过度缓存
    rerender(<PlatformCard {...baseProps} expanded={true} />);
    expect(metaSpy).toHaveBeenCalledTimes(2);
  });
});

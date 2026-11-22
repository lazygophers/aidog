// PlatformDetail 行为测试（T2 骨架）：渲染真组件、断言走 data-testid。
// 改测行为（渲染了什么 + 点击派发什么），禁快照/className 断言。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "../../test/render";
import { PlatformDetail } from "./PlatformDetail";
import type { Platform } from "../../services/api";
import type { PlatformCardService } from "../../components/platforms/PlatformCard";

// Mock Tauri invoke
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// WaterLevel（T3 数据板块）经 SSE 传输层订阅；jsdom 无 EventSource，桩成空实现（不外呼）。
vi.stubGlobal("EventSource", class { onerror = null; addEventListener() {} removeEventListener() {} close() {} });

vi.mock("../../domains/platforms/useProtocolLogo", () => ({
  useProtocolLogo: vi.fn().mockReturnValue({ logoSrc: null }),
}));

// useProtocolMeta 聚合派生：mock 直返同步值
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

// 固定 Date.now 避免时间相关断言波动
const FIXED_NOW = 1_740_000_000_000;
vi.spyOn(Date, "now").mockReturnValue(FIXED_NOW);

describe("PlatformDetail", () => {
  const platform = {
    id: 1,
    name: "Test Platform",
    platform_type: "anthropic",
    base_url: "https://api.anthropic.com",
    api_key: "sk-test",
    extra: JSON.stringify({
      allowed_clients: ["claude_code"],
      rate_limits: { rpm: 60 },
    }),
    models: { default: ["m1", "m2"], opus: ["o1"] },
    available_models: [],
    endpoints: [{ protocol: "anthropic", base_url: "https://api.anthropic.com" }],
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
  } as Platform;

  const service: PlatformCardService = {
    onToggleEnabled: vi.fn(),
    onEdit: vi.fn(),
    onShare: vi.fn(),
    onDuplicate: vi.fn(),
    onViewLogs: vi.fn(),
    onQuickTest: vi.fn(),
    onCustomTest: vi.fn(),
    onDetail: vi.fn(),
  };
  const onBack = vi.fn();
  const onRefreshQuota = vi.fn();
  const baseProps = { platform, membership: ["grp1"], onBack, onRefreshQuota, service };

  beforeEach(() => {
    Object.values(service).forEach(fn => fn.mockClear());
    onBack.mockClear();
    onRefreshQuota.mockClear();
  });

  it("页头: 渲染平台名、健康三态灯、启停开关、协议副行", () => {
    render(<PlatformDetail {...baseProps} />);
    expect(screen.getByText("Test Platform")).toBeInTheDocument();
    expect(screen.getByTestId("detail-health-dot")).toBeInTheDocument();
    expect(screen.getByTestId("detail-toggle")).toBeInTheDocument();
    // 协议副行（页头与配置端点行都含协议名，取多个匹配）
    expect(screen.getAllByText(/Anthropic/).length).toBeGreaterThan(0);
  });

  it("返回: 点击返回按钮派发 onBack", () => {
    render(<PlatformDetail {...baseProps} />);
    fireEvent.click(screen.getByTestId("detail-back"));
    expect(onBack).toHaveBeenCalledTimes(1);
  });

  it("启停: 点击开关派发 onToggleEnabled(platform)", () => {
    render(<PlatformDetail {...baseProps} />);
    fireEvent.click(screen.getByTestId("detail-toggle"));
    expect(service.onToggleEnabled).toHaveBeenCalledWith(platform);
  });

  it("快捷操作: 测连通/查余额/跳日志/编辑平台 各派发对应 handler", () => {
    render(<PlatformDetail {...baseProps} />);
    fireEvent.click(screen.getByTestId("detail-act-test"));
    expect(service.onQuickTest).toHaveBeenCalledWith(platform);
    fireEvent.click(screen.getByTestId("detail-act-quota"));
    expect(onRefreshQuota).toHaveBeenCalledWith(platform);
    fireEvent.click(screen.getByTestId("detail-act-logs"));
    expect(service.onViewLogs).toHaveBeenCalledWith(platform);
    fireEvent.click(screen.getByTestId("detail-act-edit"));
    expect(service.onEdit).toHaveBeenCalledWith(platform);
  });

  it("快捷操作: 拉模型按钮在场（点击走 invoke，不在此断言网络行为）", () => {
    render(<PlatformDetail {...baseProps} />);
    expect(screen.getByTestId("detail-act-fetch")).toBeInTheDocument();
  });

  it("配置(只读): 端点/模型槽位链/限频/白名单/分组 渲染", () => {
    render(<PlatformDetail {...baseProps} />);
    expect(screen.getByTestId("detail-config")).toBeInTheDocument();
    // 模型槽位链：default 链 m1(主) m2；opus 槽 o1
    expect(screen.getByText("m1")).toBeInTheDocument();
    expect(screen.getByText("m2")).toBeInTheDocument();
    expect(screen.getByText("o1")).toBeInTheDocument();
    // 限频摘要（extra.rate_limits rpm=60）
    expect(screen.getByText("60 RPM")).toBeInTheDocument();
    // 客户端白名单码
    expect(screen.getByText("claude_code")).toBeInTheDocument();
    // 所属分组（页头徽标 + 配置行两处渲染）
    expect(screen.getAllByText("grp1").length).toBeGreaterThan(0);
    // 端点（页头副行 + 配置行两处渲染）
    expect(screen.getAllByText(/https:\/\/api.anthropic.com/).length).toBeGreaterThan(0);
  });

  it("T3 数据板块挂载点在场", () => {
    render(<PlatformDetail {...baseProps} />);
    expect(screen.getByTestId("detail-t3-mount")).toBeInTheDocument();
  });
});

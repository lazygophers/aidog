// GroupDetailPage 就近测试：三栏挂载 + 成员行 + 切组 + 趋势卡。
// 选择器锚稳定 testid；文案断言用 i18n key（test/render 回退 key）。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent } from "@testing-library/react";
import { render, screen, waitFor } from "../../test/render";
import type { GroupDailyBreakdown } from "../../services/api";

vi.mock("../../services/api", async () => {
  const actual = await vi.importActual<typeof import("../../services/api")>("../../services/api");
  return {
    ...actual,
    groupFlowApi: {
      breakdown: vi.fn().mockResolvedValue([]),
      latestRouting: vi.fn().mockResolvedValue(null),
      daily: vi.fn(),
    },
  };
});

import { groupFlowApi } from "../../services/api";
import { GroupDetailPage } from "./GroupDetailPage";

const daily: GroupDailyBreakdown[] = [
  { day: "2026-10-25", platform_id: 1, requests: 100, est_cost: 0.5 },
  { day: "2026-10-25", platform_id: 2, requests: 40, est_cost: 0.2 },
  { day: "2026-10-26", platform_id: 1, requests: 60, est_cost: 0.3 },
];

const detail = {
  group: { id: 1, name: "main", group_key: "gk", routing_mode: "failover", is_default: true },
  platforms: [
    { platform: { id: 1, platform_type: "glm_coding", status: "enabled" }, priority: 1, weight: 1, level_priority: 8 },
    { platform: { id: 2, platform_type: "deepseek", status: "auto_disabled" }, priority: 2, weight: 1, level_priority: 5 },
  ],
  model_mappings: [{ source_model: "gpt-5.2", target_platform_id: 2, target_model: "deepseek-v4", request_timeout_secs: 60, connect_timeout_secs: 10 }],
} as never;
const details = [detail] as never;
const platforms = [
  { id: 1, platform_type: "glm_coding", status: "enabled", endpoints: [{ protocol: "openai" }] },
  { id: 2, platform_type: "deepseek", status: "auto_disabled", endpoints: [] },
] as never;
const cards = {
  quotaMap: { 1: { success: true, error: null, queried_at: 0, balance: { remaining: 30, total: 100, used: 70, currency: "CNY", is_valid: true }, coding_plan: null } },
  quotaRealIds: {}, quotaRefreshing: {}, usageMap: {
    1: { total_requests: 800, success_count: 790, total_input_tokens: 1, total_output_tokens: 1, total_cache_tokens: 0, cache_rate: 0, recent_failures: 0, recent_total: 0, total_cost: 3.2, today_tokens: 1248, today_cost: 4.8 },
    2: { total_requests: 200, success_count: 100, total_input_tokens: 1, total_output_tokens: 1, total_cache_tokens: 0, cache_rate: 0, recent_failures: 9, recent_total: 10, total_cost: 0.4, today_tokens: 56, today_cost: 0.03 },
  },
  expandedIds: new Set<number>(), testResults: {}, testingId: null, faviconFailed: new Set<number>(), lastTestMap: { 1: { success: true, status_code: 200, duration_ms: 342, created_at: 0, error: "", response_body: "" } },
} as never;

const baseProps = {
  detail, details, platforms,
  t: ((k: string) => k) as never,
  groupStats: { gk: { total_requests: 1000, success_count: 890, total_input_tokens: 1, total_output_tokens: 1, total_cache_tokens: 0, cache_rate: 0, recent_failures: 0, recent_total: 0, total_cost: 4.83, today_tokens: 1248, today_cost: 4.8 } } as never,
  cards,
  cardService: { onQuickTest: vi.fn(), onEdit: vi.fn(), onShare: vi.fn(), onDuplicate: vi.fn(), onViewLogs: vi.fn(), onCustomTest: vi.fn(), onToggleEnabled: vi.fn() } as never,
  groupTestRunning: false,
  onSelectGroup: vi.fn(), onClose: vi.fn(), onEditGroup: vi.fn(),
  onTestGroup: vi.fn(), onEditPlatform: vi.fn(),
  onDeleteMapping: vi.fn(), onAddPlatform: vi.fn(), onSetLevelPriority: vi.fn(),
};

describe("GroupDetailPage — 观测台三栏", () => {
  beforeEach(() => vi.clearAllMocks());

  it("双栏挂载：统计带 / flow / 成员表 / 趋势", async () => {
    vi.mocked(groupFlowApi.daily!).mockResolvedValue(daily);
    render(<GroupDetailPage {...baseProps} />);
    expect(document.querySelector('[data-testid="detail-group-list"]')).toBeNull(); // 左栏 GROUPS 已移除
    expect(document.querySelector('[data-testid="detail-stats"]')).toBeTruthy();
    await waitFor(() => expect(document.querySelector('[data-testid="detail-flow"]')).toBeTruthy());
    expect(document.querySelector('[data-testid="detail-members"]')).toBeTruthy();
    // 成员行：两平台 + 熔断徽标 + 协议小字
    expect(document.querySelector('[data-plat-id="1"]')!.textContent).toContain("glm_coding");
    expect(document.querySelector('[data-plat-id="1"]')!.textContent).toContain("openai");
    expect(document.querySelector('[data-plat-id="2"]')!.textContent).toContain("group.detail.broken");
    // 模型映射 chip 链
    expect(document.querySelector('[data-testid="detail-mappings"]')!.textContent).toContain("gpt-5.2");
    // 趋势卡拿到 daily 后渲染两排柱（top2 平台）
    await waitFor(() => expect(document.querySelector('[data-testid="detail-trend"] [title*="2026-10-25"]')).toBeTruthy());
  });

  it("返回 → onClose", () => {
    vi.mocked(groupFlowApi.daily!).mockResolvedValue([]);
    render(<GroupDetailPage {...baseProps} />);
    fireEvent.click(document.querySelector('[data-testid="detail-back"]')!);
    expect(baseProps.onClose).toHaveBeenCalled();
  });

  it("映射删除按钮 → onDeleteMapping(groupId, index)", () => {
    vi.mocked(groupFlowApi.daily!).mockResolvedValue([]);
    render(<GroupDetailPage {...baseProps} />);
    const del = document.querySelector('[data-testid="detail-mappings"] button')!;
    fireEvent.click(del);
    expect(baseProps.onDeleteMapping).toHaveBeenCalledWith(1, 0);
  });
});

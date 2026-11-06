// GroupListItem 就近测试：showLastOutcome 透传（分组列表与详情平台功能对齐，差距一）。
// 选择器锚稳定 testid + data-result；不断言 locale 文案。
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "../../test/render";

// 币种 store 首次订阅的 refresh 固定回 USD（空操作）。
vi.mock("../../services/api/currency", () => ({
  DISPLAY_CURRENCY_EVENT: "display-currency-updated",
  displayCurrencyApi: { get: vi.fn().mockResolvedValue({ code: "USD", symbol: "$", decimals: 2, rate: 1, table: [] }) },
}));

import { GroupListItem } from "./GroupListItem";
import type { CardsSnapshot } from "./GroupListItem";
import type { GroupDetail, Platform } from "../../services/api";

const platform = {
  id: 1, name: "P1", platform_type: "deepseek", base_url: "https://x.com",
  extra: "{}", models: {}, available_models: [], endpoints: [], status: "enabled",
  est_balance_remaining: 1, balance_level: "", manual_budgets: [],
  last_error: "HTTP 429 rate limited", last_error_at: Date.now() - 60_000,
} as never as Platform;

const detail = {
  group: { id: 3, name: "g", group_key: "k", routing_mode: "failover", is_default: false },
  platforms: [{ platform: { id: 1 }, priority: 1, weight: 1, level_priority: 5 }],
  model_mappings: [],
} as never as GroupDetail;

const cards = {
  quotaMap: {}, quotaRealIds: {}, quotaRefreshing: {}, usageMap: {},
  expandedIds: new Set<number>(), testResults: {}, testingId: null,
  faviconFailed: new Set<number>(), lastTestMap: {},
} as never as CardsSnapshot;

const noop = () => {};
const handle = { ref: noop, attributes: {}, listeners: undefined, isDragging: false } as never;

const baseProps = {
  detail, index: 0, usageStat: undefined, lastRequest: undefined, balance: undefined,
  platforms: [platform],
  isExpanded: true, isDragOver: false, dropIndicatorIdx: null, dropIndicatorTotal: 1,
  visiblePlatformIds: null, forceExpanded: false, groupTestRunning: false,
  cards, actions: {} as never, cardService: {} as never,
  t: ((k: string, f?: string) => f ?? k) as never,
  onToggleExpanded: noop, onSetCollapsed: noop, onEdit: noop, onDelete: noop,
  onToggleDefault: noop, onTestGroup: noop, onPlatPointerDown: noop,
  onSetLevelPriority: noop, onPurgeDisabled: noop,
  onBatchDelete: noop, onBatchOverrideModels: noop, onBatchSetStatus: noop, onBatchMoveGroup: noop,
  handle,
};

describe("GroupListItem — showLastOutcome 透传（对齐详情页）", () => {
  it("成员行带最近请求结果：last_error 非空 → data-result=fail 且原因可见", () => {
    render(<GroupListItem {...baseProps} />);
    const el = screen.getByTestId("row-last-outcome");
    expect(el.getAttribute("data-result")).toBe("fail");
    expect(screen.getByTestId("row-last-outcome-reason").textContent).toContain("HTTP 429 rate limited");
  });

  it("last_error 为空 → data-result=ok", () => {
    const p = { ...platform, last_error: "", last_error_at: 0 } as never as Platform;
    render(<GroupListItem {...baseProps} platforms={[p]} detail={{ ...detail, platforms: [{ platform: { id: 1 }, priority: 1, weight: 1, level_priority: 5 }] } as never} />);
    expect(screen.getByTestId("row-last-outcome").getAttribute("data-result")).toBe("ok");
  });
});

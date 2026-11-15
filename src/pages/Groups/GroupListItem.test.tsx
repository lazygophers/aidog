// GroupListItem 就近测试：showLastOutcome 透传（分组列表与详情平台功能对齐，差距一）。
// 选择器锚稳定 testid + data-result；不断言 locale 文案。
import { describe, it, expect, vi, beforeAll } from "vitest";
import { render, screen, fireEvent } from "../../test/render";
import userEvent from "@testing-library/user-event";

// 币种 store 首次订阅的 refresh 固定回 USD（空操作）。
vi.mock("../../services/api/currency", () => ({
  DISPLAY_CURRENCY_EVENT: "display-currency-updated",
  displayCurrencyApi: { get: vi.fn().mockResolvedValue({ code: "USD", symbol: "$", decimals: 2, rate: 1, table: [] }) },
}));

import { GroupListItem } from "./GroupListItem";
import type { CardsSnapshot } from "./GroupListItem";
import { platformApi } from "../../services/api";
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

describe("GroupListItem — 删除分组菜单项", () => {
  const openMenu = () => fireEvent.pointerDown(screen.getByText("⋯"), { button: 0, ctrlKey: false });

  it("手动分组：点击「删除分组」回调 onDelete(group.id)", async () => {
    const onDelete = vi.fn();
    render(<GroupListItem {...baseProps} onDelete={onDelete} />);
    openMenu();
    fireEvent.click(await screen.findByText("删除分组"));
    expect(onDelete).toHaveBeenCalledWith(3);
  });

  it("auto 分组带平台：后端 delete_group 已允许（2026-10-27 拍板），菜单必须仍给「删除分组」", async () => {
    const auto = { ...detail, group: { ...detail.group, auto_from_platform: 1 } } as never as GroupDetail;
    render(<GroupListItem {...baseProps} detail={auto} />);
    openMenu();
    await screen.findByText("一键测试本组全部平台");
    expect(screen.queryByText("删除分组")).not.toBeNull();
  });
});

describe("GroupListItem — 多选「按状态选」", () => {
  const mk = (id: number, status: string) => ({ ...platform, id, name: `P${id}`, status, last_error: "", last_error_at: 0 }) as never as Platform;
  const plats = [mk(1, "enabled"), mk(2, "disabled"), mk(3, "disabled"), mk(4, "auto_disabled")];
  const det = {
    ...detail,
    platforms: plats.map((p, i) => ({ platform: { id: p.id }, priority: i, weight: 1, level_priority: 5 })),
  } as never as GroupDetail;

  const checked = () =>
    screen.getAllByTestId("platform-row")
      .filter(r => r.querySelector('[role="checkbox"]')?.getAttribute("aria-checked") === "true")
      .map(r => Number(r.getAttribute("data-platform-id")));

  // jsdom 缺 Radix Select 依赖的 pointer capture / scrollIntoView。
  beforeAll(() => {
    Element.prototype.hasPointerCapture ??= () => false;
    Element.prototype.setPointerCapture ??= () => {};
    Element.prototype.releasePointerCapture ??= () => {};
    Element.prototype.scrollIntoView ??= () => {};
  });

  it("选「禁用」→ 只勾选 status=disabled 的平台；再选「启用」可重新触发", async () => {
    vi.spyOn(platformApi, "errorStatus").mockResolvedValue([]);
    render(<GroupListItem {...baseProps} platforms={plats} detail={det} />);
    fireEvent.click(screen.getByTestId("group-batch-start"));
    const user = userEvent.setup();
    await user.click(screen.getByTestId("group-batch-status-select"));
    await user.click(await screen.findByTestId("group-batch-status-opt-s:disabled"));
    expect(checked().sort()).toEqual([2, 3]);

    await user.click(screen.getByTestId("group-batch-status-select"));
    await user.click(await screen.findByTestId("group-batch-status-opt-s:enabled"));
    expect(checked()).toEqual([1]);
  });
});

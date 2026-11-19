// Groups.test.tsx — GroupsEmbedded 列表→详情全链（复现 2026-10-27 用户报「点第 N 组详情，展示的是第一组」）。
// 选择器锚稳定 testid；组名断言用 fixture 字面值（组名非 i18n 文案）。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "../test/render";

const plat = { id: 11, name: "P11", platform_type: "openai", base_url: "https://example.test", status: "enabled", enabled: true, models: {}, extra: "{}" };
const g1 = {
  group: { id: 1, name: "First", group_key: "k1", routing_mode: "failover", is_default: false },
  platforms: [{ platform: plat, priority: 1, weight: 1, level_priority: 5 }], model_mappings: [],
};
const g2 = {
  group: { id: 2, name: "Work", group_key: "k2", routing_mode: "failover", is_default: false },
  platforms: [], model_mappings: [],
};
const g3 = {
  group: { id: 3, name: "Work Two", group_key: "k3", routing_mode: "failover", is_default: false },
  platforms: [], model_mappings: [],
};
const allGroups = [g1, g2, g3, ...Array.from({ length: 10 }, (_, i) => ({
  group: { id: i + 4, name: `Group ${i + 4}`, group_key: `k${i + 4}`, routing_mode: "failover", is_default: false },
  platforms: [], model_mappings: [],
}))];

vi.mock("../services/api", async () => {
  const actual = await vi.importActual<typeof import("../services/api")>("../services/api");
  return {
    ...actual,
    groupDetailApi: {
      ...actual.groupDetailApi,
      listPaged: vi.fn(async (offset: number, limit: number) => allGroups.slice(offset, offset + limit)),
      list: vi.fn(async () => allGroups),
    },
    groupApi: { ...actual.groupApi, reorder: vi.fn(async () => {}) },
    platformApi: { ...actual.platformApi, list: vi.fn(async () => [plat]) },
    groupUsageApi: { ...actual.groupUsageApi, statsAll: vi.fn(async () => ({})) },
    proxyApi: { ...actual.proxyApi, getSettings: vi.fn(async () => ({ port: 9890 })) },
    groupFlowApi: { ...actual.groupFlowApi, daily: vi.fn(async () => []), flow: vi.fn(async () => []), latestRouting: vi.fn(async () => null) },
    onProxyLogUpdated: vi.fn(() => () => {}),
  };
});
// 协议搜索词挂载拉取走 defaults docPromise（tauri IPC，jsdom 不可用）→ 空表 mock。
vi.mock("../domains/platforms/defaults", async () => {
  const actual = await vi.importActual<typeof import("../domains/platforms/defaults")>("../domains/platforms/defaults");
  return { ...actual, getProtocolSearchTermsMap: vi.fn(async () => ({})) };
});
vi.mock("../components/SortableList", () => ({
  SortableList: ({ items, onReorder, renderItem }: {
    items: { id: string }[];
    onReorder: (items: { id: string }[]) => void;
    renderItem: (item: { id: string }, handle: { ref: () => void; attributes: {}; listeners: { onPointerDown: (e: React.PointerEvent) => void }; isDragging: false }) => React.ReactNode;
  }) => <>
    {items.map(item => <div key={item.id}>{renderItem(item, {
      ref: () => {}, attributes: {}, isDragging: false,
      listeners: { onPointerDown: (e: React.PointerEvent) => { (e.currentTarget as HTMLElement).setAttribute("data-drag-activator", "true"); } },
    })}</div>)}
    <button data-testid="sortable-reorder" onClick={() => onReorder([...items].reverse())}>reorder</button>
  </>,
}));

import { GroupsEmbedded } from "./Groups";
import { groupApi } from "../services/api";

describe("GroupsEmbedded — 列表→详情全链", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.stubGlobal("IntersectionObserver", class { observe() {} disconnect() {} unobserve() {} });
  });

  it("点第二组「详情」→ 详情页展示第二组（data-group-id=2 + pgtop 组名）", async () => {
    render(<GroupsEmbedded onToast={() => {}} />);
    const btns = await waitFor(() => {
      const b = screen.getAllByTestId("group-section-detail");
      expect(b.length).toBe(12);
      return b;
    });
    fireEvent.click(btns[1]);
    await waitFor(() => expect(document.querySelector('[data-testid="detail-members"]')).toBeTruthy());
    expect(document.querySelector('[data-testid="detail-members"]')!.getAttribute("data-group-id")).toBe("2");
    expect(screen.getByText("Work")).toBeTruthy();
  });

  it("搜索命中的分组可排序，保存时保留完整 ID 顺序", async () => {
    render(<GroupsEmbedded onToast={() => {}} searchQuery="Work" />);
    await waitFor(() => expect(screen.getAllByTestId("group-section-detail").length).toBe(2));
    fireEvent.click(screen.getByTestId("sortable-reorder"));
    await waitFor(() => expect(groupApi.reorder).toHaveBeenCalledWith([1, 3, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]));
  });

  it("排序保存失败时显示错误", async () => {
    const onToast = vi.fn();
    vi.mocked(groupApi.reorder).mockRejectedValueOnce(new Error("isolated failure"));
    render(<GroupsEmbedded onToast={onToast} searchQuery="Work" />);
    await waitFor(() => expect(screen.getAllByTestId("group-section-detail").length).toBe(2));
    fireEvent.click(screen.getByTestId("sortable-reorder"));
    await waitFor(() => expect(onToast).toHaveBeenCalledWith(expect.objectContaining({ ok: false, text: expect.stringContaining("isolated failure") })));
  });

  // 分组拖拽只由标题区的可见手柄启动，平台行继续使用自己的拖拽把手。
  it("标题区可见手柄启动分组排序，平台拖拽把手不触发分组排序", async () => {
    render(<GroupsEmbedded onToast={() => {}} />);
    const head = await screen.findAllByTestId("group-section-head");
    const name = await screen.findAllByTestId("group-section-name");
    const groupHandle = await screen.findAllByTestId("group-drag-handle");
    const section = head[0].closest<HTMLElement>("[data-group-id]")!;
    if (section.getAttribute("data-expanded") !== "true") fireEvent.click(head[0]);
    const platformHandle = section.querySelector(".drag-handle-inline");

    fireEvent.pointerDown(head[0]);
    expect(head[0].getAttribute("data-drag-activator")).toBeNull();
    fireEvent.pointerDown(name[0]);
    expect(groupHandle[0].getAttribute("data-drag-activator")).toBe("true");
    fireEvent.pointerDown(platformHandle!);
    expect(groupHandle[0].getAttribute("data-drag-activator")).toBe("true");
  });

  // 回归：列表分支漏渲染 sharedModals（fa236ce94），多选工具栏「改状态」点了无弹窗。
  it("列表视图：多选 → 全选 → 改状态 弹出批量改状态 modal", async () => {
    render(<GroupsEmbedded onToast={() => {}} />);
    await waitFor(() => expect(screen.getAllByTestId("group-section-detail").length).toBe(12));
    fireEvent.click(await screen.findByTestId("group-batch-start"));
    fireEvent.click(await screen.findByText(/selectAll|全选|Select all/i));
    fireEvent.click(await screen.findByText(/batchSetStatus$|改状态|Set status/i));
    expect(await screen.findByText(/batchSetStatusTitle|批量改状态|Batch set status/i)).toBeTruthy();
  });
});

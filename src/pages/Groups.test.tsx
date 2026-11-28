// Groups.test.tsx — GroupsEmbedded 列表→详情全链（复现 2026-10-27 用户报「点第 N 组详情，展示的是第一组」）。
// 选择器锚稳定 testid；组名断言用 fixture 字面值（组名非 i18n 文案）。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "../test/render";

const plat = { id: 11, name: "P11", platform_type: "openai", base_url: "https://example.test", status: "enabled", enabled: true, models: {}, extra: "{}" };
// 批量操作四流程（票 08）+ 平台列表刷新计数的可控 mock。
const listMock = vi.hoisted(() => vi.fn(async () => [plat, plat2]));
const batchMocks = vi.hoisted(() => ({
  batchDelete: vi.fn(),
  batchOverrideModels: vi.fn(),
  batchSetStatus: vi.fn(),
  batchMoveGroup: vi.fn(),
}));
const plat2 = { ...plat, id: 12, name: "P12" };
const g1 = {
  group: { id: 1, name: "First", group_key: "k1", routing_mode: "failover", is_default: false },
  platforms: [plat, plat2].map((platform, i) => ({ platform, priority: i + 1, weight: 1, level_priority: 5 })), model_mappings: [],
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
      reorderPlatforms: vi.fn(async () => {}),
    },
    groupApi: { ...actual.groupApi, reorder: vi.fn(async () => {}) },
    platformApi: { ...actual.platformApi, list: listMock, ...batchMocks },
    groupUsageApi: { ...actual.groupUsageApi, statsAll: vi.fn(async () => ({})) },
    proxyApi: { ...actual.proxyApi, getSettings: vi.fn(async () => ({ port: 9890 })) },
    groupFlowApi: { ...actual.groupFlowApi, daily: vi.fn(async () => []), flow: vi.fn(async () => []), latestRouting: vi.fn(async () => null) },
    onProxyLogUpdated: vi.fn(() => () => {}),
  };
});
// Radix Select 在 jsdom 无法开面板：换原生 select 壳（LogSettingsSection.test 同款）。
// 供「批量移组」目标组下拉用；不触碰本文件其它非 Select 交互。
vi.mock("@/components/ui/select", () => ({
  Select: ({ value, onValueChange, children }: { value: string; onValueChange: (v: string) => void; children: unknown }) => (
    <select data-testid="native-select" value={value} onChange={(e) => onValueChange(e.target.value)}>{children}</select>
  ),
  SelectTrigger: ({ children }: { children?: React.ReactNode }) => <>{children}</>,
  SelectValue: () => null,
  SelectContent: ({ children }: { children?: React.ReactNode }) => <>{children}</>,
  SelectItem: ({ value, children }: { value: string; children?: React.ReactNode }) => <option value={value}>{children}</option>,
}));

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
import { groupApi, groupDetailApi } from "../services/api";

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

  it("组内平台首项拖到末尾时保存新顺序", async () => {
    render(<GroupsEmbedded onToast={() => {}} />);
    const section = await waitFor(() => {
      const found = document.querySelector<HTMLElement>("[data-group-id='1']");
      expect(found).not.toBeNull();
      return found!;
    });
    if (section.getAttribute("data-expanded") !== "true") fireEvent.click(section.querySelector("[data-testid='group-section-chev']")!);
    const rows = [...section.querySelectorAll<HTMLElement>("[data-testid='platform-row']")];
    expect(rows.map(row => row.dataset.platformId)).toEqual(["11", "12"]);
    rows.forEach((row, i) => {
      row.getBoundingClientRect = () => ({ x: 0, y: i * 40, top: i * 40, bottom: (i + 1) * 40, left: 0, right: 200, width: 200, height: 40, toJSON: () => ({}) });
    });
    const originalElementFromPoint = document.elementFromPoint;
    Object.defineProperty(document, "elementFromPoint", { configurable: true, value: () => rows[1] });
    try {
      fireEvent.pointerDown(section.querySelector(".drag-handle-inline")!, { clientX: 1, clientY: 1 });
      fireEvent.pointerMove(document, { clientX: 20, clientY: 100 });
      fireEvent.pointerUp(document, { clientX: 20, clientY: 100 });
      await waitFor(() => expect(groupDetailApi.reorderPlatforms).toHaveBeenCalledWith(1, [12, 11]));
    } finally {
      if (originalElementFromPoint) Object.defineProperty(document, "elementFromPoint", { configurable: true, value: originalElementFromPoint });
      else Reflect.deleteProperty(document, "elementFromPoint");
    }
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

// ── 批量操作四流程行为锁定（票 08）：成功 → target 清 / 刷新 / 成功 toast；失败 → target 清 / 失败 toast / 不刷新。
describe("批量操作四流程行为锁定（票 08）", () => {
  beforeAll(() => {
    Element.prototype.hasPointerCapture ??= () => false;
    Element.prototype.setPointerCapture ??= () => {};
    Element.prototype.releasePointerCapture ??= () => {};
    Element.prototype.scrollIntoView ??= () => {};
  });

  async function openBatchBar() {
    const onToast = vi.fn();
    render(<GroupsEmbedded onToast={onToast} />);
    await waitFor(() => expect(screen.getAllByTestId("group-section-detail").length).toBe(12));
    fireEvent.click(await screen.findByTestId("group-batch-start"));
    fireEvent.click(await screen.findByText(/selectAll|全选|Select all/i));
    return onToast;
  }

  it.each([
    ["删除", "group.batchDelete", "group.batchDeleteTitle"],
    ["覆盖模型", "group.batchOverrideModels", "group.batchOverrideModelsTitle"],
    ["改状态", "group.batchSetStatus", "group.batchSetStatusTitle"],
  ] as [string, string, string][])("%s：成功 → toast ok / modal 关 / 触发刷新", async (_name, toolText, titleKey) => {
    const onToast = await openBatchBar();
    fireEvent.click(await screen.findByText(toolText));
    expect(await screen.findByText(titleKey)).toBeTruthy();
    if (toolText === "group.batchOverrideModels") {
      fireEvent.change(await screen.findByTestId("batch-slot-default"), { target: { value: "m1" } });
    }
    const reloads = listMock.mock.calls.length;
    if (toolText === "group.batchDelete") batchMocks.batchDelete.mockResolvedValueOnce({ applied: 2 });
    if (toolText === "group.batchOverrideModels") batchMocks.batchOverrideModels.mockResolvedValueOnce({ applied: 2 });
    if (toolText === "group.batchSetStatus") batchMocks.batchSetStatus.mockResolvedValueOnce({ applied: 2 });
    fireEvent.click(await screen.findByText(/Confirm|确定/));
    await waitFor(() => expect(onToast).toHaveBeenLastCalledWith(expect.objectContaining({ ok: true })));
    expect(screen.queryByText(titleKey)).toBeNull();
    expect(listMock.mock.calls.length).toBeGreaterThan(reloads);
  });

  it.each([
    ["删除", "group.batchDelete", "group.batchDeleteTitle", "group.batchDeleteFailed", batchMocks.batchDelete],
    ["覆盖模型", "group.batchOverrideModels", "group.batchOverrideModelsTitle", "group.batchOverrideModelsFailed", batchMocks.batchOverrideModels],
    ["改状态", "group.batchSetStatus", "group.batchSetStatusTitle", "group.batchSetStatusFailed", batchMocks.batchSetStatus],
  ] as [string, string, string, string, ReturnType<typeof vi.fn>][])("%s：失败 → toast fail / modal 关 / 不刷新", async (_name, toolText, titleKey, failKey, mockFn) => {
    const onToast = await openBatchBar();
    fireEvent.click(await screen.findByText(toolText));
    expect(await screen.findByText(titleKey)).toBeTruthy();
    if (toolText === "group.batchOverrideModels") {
      fireEvent.change(await screen.findByTestId("batch-slot-default"), { target: { value: "m1" } });
    }
    const reloads = listMock.mock.calls.length;
    mockFn.mockRejectedValueOnce(new Error("boom"));
    fireEvent.click(await screen.findByText(/Confirm|确定/));
    await waitFor(() => expect(onToast).toHaveBeenLastCalledWith(expect.objectContaining({ ok: false })));
    expect(onToast.mock.lastCall![0].text).toContain(failKey);
    expect(screen.queryByText(titleKey)).toBeNull();
    expect(listMock.mock.calls.length).toBe(reloads);
  });

  /** 移组弹窗的目标组下拉：页面有多个原生 select 壳，按 option 值域（组 id）定位。 */
  const moveGroupSelect = () =>
    screen.getAllByTestId("native-select").find(s => Array.from((s as HTMLSelectElement).options).some(o => o.value === "2"))!;

  it("移组：选目标组 → 成功 → toast ok / modal 关 / 触发刷新", async () => {
    const onToast = await openBatchBar();
    fireEvent.click(await screen.findByText("group.batchMoveGroup"));
    expect(await screen.findByText("group.batchMoveGroupTitle")).toBeTruthy();
    fireEvent.change(moveGroupSelect(), { target: { value: "2" } });
    const reloads = listMock.mock.calls.length;
    batchMocks.batchMoveGroup.mockResolvedValueOnce({ applied: 2 });
    fireEvent.click(await screen.findByText(/batchMoveGroupConfirm/));
    await waitFor(() => expect(onToast).toHaveBeenLastCalledWith(expect.objectContaining({ ok: true })));
    expect(screen.queryByText("group.batchMoveGroupTitle")).toBeNull();
    expect(listMock.mock.calls.length).toBeGreaterThan(reloads);
  });

  it("移组：失败 → toast fail / modal 关 / 不刷新", async () => {
    const onToast = await openBatchBar();
    fireEvent.click(await screen.findByText("group.batchMoveGroup"));
    expect(await screen.findByText("group.batchMoveGroupTitle")).toBeTruthy();
    fireEvent.change(moveGroupSelect(), { target: { value: "2" } });
    const reloads = listMock.mock.calls.length;
    batchMocks.batchMoveGroup.mockRejectedValueOnce(new Error("boom"));
    fireEvent.click(await screen.findByText(/batchMoveGroupConfirm/));
    await waitFor(() => expect(onToast).toHaveBeenLastCalledWith(expect.objectContaining({ ok: false })));
    expect(onToast.mock.lastCall![0].text).toContain("group.batchMoveGroupFailed");
    expect(screen.queryByText("group.batchMoveGroupTitle")).toBeNull();
    expect(listMock.mock.calls.length).toBe(reloads);
  });
});

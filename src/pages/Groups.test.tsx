// Groups.test.tsx — GroupsEmbedded 列表→详情全链（复现 2026-10-27 用户报「点第 N 组详情，展示的是第一组」）。
// 选择器锚稳定 testid；组名断言用 fixture 字面值（组名非 i18n 文案）。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "../test/render";

const g1 = {
  group: { id: 1, name: "First", group_key: "k1", routing_mode: "failover", is_default: false },
  platforms: [], model_mappings: [],
};
const g2 = {
  group: { id: 2, name: "Work", group_key: "k2", routing_mode: "failover", is_default: false },
  platforms: [], model_mappings: [],
};

vi.mock("../services/api", async () => {
  const actual = await vi.importActual<typeof import("../services/api")>("../services/api");
  return {
    ...actual,
    groupDetailApi: { ...actual.groupDetailApi, listPaged: vi.fn(async (offset: number) => (offset === 0 ? [g1, g2] : [])) },
    platformApi: { ...actual.platformApi, list: vi.fn(async () => []) },
    groupUsageApi: { ...actual.groupUsageApi, statsAll: vi.fn(async () => ({})) },
    proxyApi: { ...actual.proxyApi, getSettings: vi.fn(async () => ({ port: 9890 })) },
    groupFlowApi: { ...actual.groupFlowApi, daily: vi.fn(async () => []), breakdown: vi.fn(async () => []), latestRouting: vi.fn(async () => null) },
    onProxyLogUpdated: vi.fn(() => () => {}),
  };
});
// 协议搜索词挂载拉取走 defaults docPromise（tauri IPC，jsdom 不可用）→ 空表 mock。
vi.mock("../domains/platforms/defaults", async () => {
  const actual = await vi.importActual<typeof import("../domains/platforms/defaults")>("../domains/platforms/defaults");
  return { ...actual, getProtocolSearchTermsMap: vi.fn(async () => ({})) };
});

import { GroupsEmbedded } from "./Groups";

describe("GroupsEmbedded — 列表→详情全链", () => {
  beforeEach(() => vi.clearAllMocks());

  it("点第二组「详情」→ 详情页展示第二组（data-group-id=2 + pgtop 组名）", async () => {
    render(<GroupsEmbedded onToast={() => {}} />);
    const btns = await waitFor(() => {
      const b = screen.getAllByTestId("group-section-detail");
      expect(b.length).toBe(2);
      return b;
    });
    fireEvent.click(btns[1]);
    await waitFor(() => expect(document.querySelector('[data-testid="detail-members"]')).toBeTruthy());
    expect(document.querySelector('[data-testid="detail-members"]')!.getAttribute("data-group-id")).toBe("2");
    expect(screen.getByText("Work")).toBeTruthy();
  });
});

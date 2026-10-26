// G3 分组页就近测试（票 03）：桑基 flow 渲染 + 段切换 + 候选序/最近路由。
// 选择器锚稳定 id + data-state；文案断言用 i18n key（test/render 回退 key）。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent } from "@testing-library/react";
import { render, screen, waitFor } from "../../test/render";
import type { GroupPlatformBreakdown, LatestRouting } from "../../services/api";

vi.mock("../../services/api", async () => {
  const actual = await vi.importActual<typeof import("../../services/api")>("../../services/api");
  return {
    ...actual,
    groupFlowApi: {
      breakdown: vi.fn(),
      latestRouting: vi.fn(),
    },
  };
});

import { groupFlowApi } from "../../services/api";
import { GroupFlowPanel } from "./GroupFlowPanel";
import { GroupSidePanels } from "./GroupSidePanels";

const rows: GroupPlatformBreakdown[] = [
  { platform_id: 1, model: "m-a", requests: 10, success: 10, est_cost: 0.1 },
  { platform_id: 2, model: "m-b", requests: 5, success: 5, est_cost: 0.5 },
];
const gps = [
  { platform: { id: 1, platform_type: "glm_coding", status: "enabled" }, priority: 1, weight: 1, level_priority: 8 },
  { platform: { id: 2, platform_type: "deepseek", status: "enabled" }, priority: 2, weight: 1, level_priority: 5 },
  { platform: { id: 3, platform_type: "kimi", status: "disabled" }, priority: 3, weight: 1, level_priority: 10 },
] as never;

describe("GroupFlowPanel — 桑基（票 03）", () => {
  beforeEach(() => vi.clearAllMocks());

  it("渲染模型/平台两列 + 连线（path 数 = 明细行数）", async () => {
    vi.mocked(groupFlowApi.breakdown).mockResolvedValue(rows);
    render(<GroupFlowPanel groupKey="gk" gps={gps} />);
    const svg = await waitFor(() => {
      const el = document.getElementById("g3-flow-svg")!;
      expect(el).toBeTruthy();
      return el;
    });
    expect(svg.querySelectorAll("path")).toHaveLength(2);
    // 段切换：today 触发第二次拉取
    const segs = document.querySelectorAll('[data-state]');
    const today = [...segs].find(el => el.textContent === "group.flow.today")!;
    fireEvent.click(today);
    expect(groupFlowApi.breakdown).toHaveBeenCalledTimes(2);
  });

  it("空数据 → 空态", async () => {
    vi.mocked(groupFlowApi.breakdown).mockResolvedValueOnce([]);
    render(<GroupFlowPanel groupKey="gk" gps={gps} />);
    await screen.findByText("group.flow.empty");
  });
});

describe("GroupSidePanels — 候选序 + 最近路由（票 03）", () => {
  beforeEach(() => vi.clearAllMocks());

  it("静态序过滤停用平台并按 level_priority 降序；下一个请求指向首位", async () => {
    vi.mocked(groupFlowApi.latestRouting).mockResolvedValueOnce(null);
    render(<GroupSidePanels group={{ id: 1, group_key: "gk", routing_mode: "failover" } as never} gps={gps} />);
    expect(document.getElementById("g3-cand-3")).toBeNull(); // disabled 不参与
    const first = document.getElementById("g3-cand-1")!;
    expect(first.textContent).toContain("glm_coding");
    expect(document.getElementById("g3-next-request")!.textContent).toContain("glm_coding");
    await screen.findByText("group.routing.none");
  });

  it("最近路由：selected 平台 + reason 徽标", async () => {
    const latest: LatestRouting = {
      platform_id: 2, source_model: "claude-opus", actual_model: "glm-5.3", created_at: 1730000000000,
      snapshot: { mode: null, entries: [
        { order: 1, platform: "deepseek", platform_id: 2, level_priority: 1, priority: 1, reason: "selected", tags: [] },
        { order: 2, platform: "glm_coding", platform_id: 1, level_priority: 1, priority: 2, reason: "excluded_no_model", tags: [] },
      ] },
    };
    vi.mocked(groupFlowApi.latestRouting).mockResolvedValueOnce(latest);
    render(<GroupSidePanels group={{ id: 1, group_key: "gk", routing_mode: "failover" } as never} gps={gps} />);
    expect((await screen.findByText("deepseek", { selector: "#g3-latest-selected" })).textContent).toBe("deepseek");
    expect(document.querySelector('[data-reason="excluded_no_model"]')).toBeTruthy();
    // 请求模型 → 实际模型
    expect(document.getElementById("g3-latest-routing")!.textContent).toContain("claude-opus → glm-5.3");
  });
});

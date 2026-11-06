// Stats 维度排行表渲染测试：成功率列显示值 / 0 请求 / 点表头升降序。
// 选择器锚稳定 testid（dim-success-rate），不锚文案与 DOM 层级。
import { describe, it, expect, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "../test/render";
import { statsApi } from "../services/api";

const dim = (name: string, total_requests: number, success_count: number) => ({
  name, total_requests, success_count, input_tokens: 0, output_tokens: 0, cache_tokens: 0,
  cache_rate: 0, avg_duration_ms: 0, total_cost: 0,
});
const result = {
  overview: {
    total_requests: 15, success_rate: 93, total_input_tokens: 0, total_output_tokens: 0,
    total_cache_tokens: 0, cache_rate: 0, avg_duration_ms: 0, total_cost: 0,
  },
  buckets: [],
  // 默认按请求数降序：A(10) / C(5) / B(0)
  dimension_data: [dim("A", 10, 9), dim("B", 0, 0), dim("C", 5, 5)],
  available_models: [],
  series: [],
};
// 会话 tab（magpie 票 09）：一条有标题、一条空标题（显示「未命名会话」key）
const sessions = [
  { session_key: "s1", title: "帮我写个登录页", requests: 12, input_tokens: 100, output_tokens: 200, cache_tokens: 300, est_cost: 1.25, first_created_at: 0, last_created_at: 0 },
  { session_key: "s2", title: "", requests: 3, input_tokens: 0, output_tokens: 0, cache_tokens: 0, est_cost: 0, first_created_at: 0, last_created_at: 0 },
];

vi.mock("../services/api", async () => {
  const actual = await vi.importActual<typeof import("../services/api")>("../services/api");
  return {
    ...actual,
    statsApi: { ...actual.statsApi, query: vi.fn(async () => result), sessionStats: vi.fn(async () => sessions) },
    mitmStatsApi: { ...actual.mitmStatsApi, opaqueCount: vi.fn(async () => 0) },
    groupDetailApi: { ...actual.groupDetailApi, list: vi.fn(async () => []) },
    platformApi: { ...actual.platformApi, list: vi.fn(async () => []) },
    onProxyLogUpdated: vi.fn(() => () => {}),
  };
});
// 协议搜索词挂载拉取走 defaults docPromise（tauri IPC，jsdom 不可用）→ 空表 mock。
vi.mock("../domains/platforms/defaults", async () => {
  const actual = await vi.importActual<typeof import("../domains/platforms/defaults")>("../domains/platforms/defaults");
  return { ...actual, getProtocolSearchTermsMap: vi.fn(async () => ({})) };
});

import { Stats } from "./Stats";

const rateTh = () => screen.getAllByText("stats.successRate").find((e) => e.closest("th"))!;
const rates = () => screen.getAllByTestId("dim-success-rate").map((c) => c.textContent);

describe("Stats 维度排行 — 成功率列", () => {
  it("显示 90.0% / 100.0%，0 请求行显示 0.0%", async () => {
    render(<Stats />);
    await waitFor(() => expect(screen.getAllByTestId("dim-success-rate").length).toBe(3));
    expect(rates()).toEqual(["90.0%", "100.0%", "0.0%"]);
  });

  it("点成功率表头：先降序再升序", async () => {
    render(<Stats />);
    await waitFor(() => expect(screen.getAllByTestId("dim-success-rate").length).toBe(3));
    fireEvent.click(rateTh());
    expect(rates()).toEqual(["100.0%", "90.0%", "0.0%"]);
    fireEvent.click(rateTh());
    expect(rates()).toEqual(["0.0%", "90.0%", "100.0%"]);
  });
});

describe("Stats 会话 tab（magpie 票 09）", () => {
  it("渲染标题/请求/Token/花费，空标题显示未命名会话，行点击带 sessionKey 跳日志页", async () => {
    const onNavigate = vi.fn();
    render(<Stats onNavigate={onNavigate} />);
    // 主查询先落（tab 列表挂在 overview 分支内），再点会话 tab
    await waitFor(() => expect(screen.getAllByTestId("dim-success-rate").length).toBe(3));
    fireEvent.click(screen.getByRole("tab", { name: "stats.tabSessions" }));
    const rows = await waitFor(() => screen.getAllByTestId("session-row"));
    expect(rows.length).toBe(2);
    const titles = screen.getAllByTestId("session-title").map((c) => c.textContent);
    expect(titles).toEqual(["帮我写个登录页", "stats.sessionUntitled"]);
    // Token 列 = input+output+cache 合计（100+200+300=600）
    expect(rows[0].textContent).toContain("600");
    expect(rows[0].textContent).toContain("$1.25");
    fireEvent.click(rows[0]);
    expect(onNavigate).toHaveBeenCalledWith("logs", { sessionKey: "s1" });
  });

  it("空列表显示诚实空态", async () => {
    (statsApi.sessionStats as ReturnType<typeof vi.fn>).mockResolvedValue([]);
    render(<Stats />);
    // 主查询先落（tab 列表挂在 overview 分支内），再点会话 tab
    await waitFor(() => expect(screen.getAllByTestId("dim-success-rate").length).toBe(3));
    fireEvent.click(screen.getByRole("tab", { name: "stats.tabSessions" }));
    await waitFor(() => expect(screen.getByText("stats.sessionEmpty")).toBeTruthy());
  });
});

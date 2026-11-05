// Stats 维度排行表渲染测试：成功率列显示值 / 0 请求 / 点表头升降序。
// 选择器锚稳定 testid（dim-success-rate），不锚文案与 DOM 层级。
import { describe, it, expect, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "../test/render";

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

vi.mock("../services/api", async () => {
  const actual = await vi.importActual<typeof import("../services/api")>("../services/api");
  return {
    ...actual,
    statsApi: { ...actual.statsApi, query: vi.fn(async () => result) },
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

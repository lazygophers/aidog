// 首页事件刷新范围：proxy-log-updated 只重拉日志相关数据（今日 / 统计 / 平台余额 / 流向），
// 代理状态与设置（端口）不随每个请求重拉。
import { describe, it, expect, vi } from "vitest";
import { render, waitFor } from "@testing-library/react";

const fire = { cb: null as null | (() => void) };

vi.mock("../services/api", async () => {
  const actual = await vi.importActual<typeof import("../services/api")>("../services/api");
  const result = { overview: {}, buckets: [], dimension_data: [], available_models: [], series: [] };
  return {
    ...actual,
    proxyApi: { ...actual.proxyApi, status: vi.fn(async () => true), getSettings: vi.fn(async () => ({ port: 9890 })) },
    trayConfigApi: { ...actual.trayConfigApi, todayStats: vi.fn(async () => null) },
    platformApi: { ...actual.platformApi, list: vi.fn(async () => []) },
    statsApi: { ...actual.statsApi, query: vi.fn(async () => result), queryBatch: vi.fn(async () => []) },
    groupFlowApi: { ...actual.groupFlowApi, flow: vi.fn(async () => []) },
    onProxyLogUpdated: vi.fn((cb: () => void) => { fire.cb = cb; return () => {}; }),
  };
});

import { Home } from "./Home";
import { proxyApi, trayConfigApi, platformApi, statsApi, groupFlowApi } from "../services/api";

describe("Home · proxy-log-updated 刷新范围", () => {
  it("事件后重拉日志相关数据，不重拉代理状态与设置", async () => {
    render(<Home onNavigate={() => {}} />);
    await waitFor(() => expect(groupFlowApi.flow).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(statsApi.queryBatch).toHaveBeenCalledTimes(1));
    expect(proxyApi.status).toHaveBeenCalledTimes(1);
    expect(proxyApi.getSettings).toHaveBeenCalledTimes(1);

    fire.cb!();

    await waitFor(() => expect(groupFlowApi.flow).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(statsApi.queryBatch).toHaveBeenCalledTimes(2));
    expect(trayConfigApi.todayStats).toHaveBeenCalledTimes(2);
    expect(statsApi.query).toHaveBeenCalledTimes(2);
    // 平台余额（est_balance_remaining）靠此事件刷新，保留
    expect(platformApi.list).toHaveBeenCalledTimes(2);
    expect(proxyApi.status).toHaveBeenCalledTimes(1);
    expect(proxyApi.getSettings).toHaveBeenCalledTimes(1);
  });
});

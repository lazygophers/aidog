// useLogsFilters.test — c8 三段切分后 filters 段的行为回归。
// 覆盖 activeFilter 派生（exclude_sources 默认值/NO_GROUP_SENTINEL 映射/请求模型/实际模型独立带）、
// hasFilter 判定、clearFilter 复位。mock 策略同 usePlatformsState.test：整包拦截 services/api。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import { NO_GROUP_SENTINEL } from "./types";

const { platformApiMock, groupDetailApiMock, proxyLogApiMock } = vi.hoisted(() => ({
  platformApiMock: { list: vi.fn<() => Promise<any[]>>() },
  groupDetailApiMock: { list: vi.fn<() => Promise<any[]>>() },
  proxyLogApiMock: {
    listFiltered: vi.fn<() => Promise<{ items: any[]; has_more: boolean }>>(),
    distinctModels: vi.fn<(f: any, actual: boolean, n?: number) => Promise<string[]>>(),
  },
}));

vi.mock("../../services/api", () => ({
  platformApi: platformApiMock,
  groupDetailApi: groupDetailApiMock,
  proxyLogApi: proxyLogApiMock,
}));

import { useLogsFilters } from "./useLogsFilters";

describe("useLogsFilters", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    platformApiMock.list.mockResolvedValue([{ id: 1, name: "P1" }]);
    groupDetailApiMock.list.mockResolvedValue([{ group: { group_key: "g1", name: "G1" } }]);
    proxyLogApiMock.listFiltered.mockResolvedValue({ items: [], has_more: false });
    proxyLogApiMock.distinctModels.mockImplementation(async (_f, actual) => (actual ? ["glm-4"] : ["claude-3"]));
  });

  it("初始 activeFilter 只带 exclude_sources 默认值", async () => {
    const r = renderHook(() => useLogsFilters());
    await waitFor(() => expect(platformApiMock.list).toHaveBeenCalled());
    expect(r.result.current.activeFilter).toEqual({ exclude_sources: ["test", "quota", "fetch-models"] });
    expect(r.result.current.hasFilter).toBe(false);
  });

  it("filterGroup=NO_GROUP_SENTINEL 时 activeFilter.group_key 映射为空串", () => {
    const r = renderHook(() => useLogsFilters());
    act(() => r.result.current.setFilterGroup(NO_GROUP_SENTINEL));
    expect(r.result.current.activeFilter.group_key).toBe("");
    expect(r.result.current.hasFilter).toBe(true);
  });

  it("请求模型/实际模型各自独立，可同时带在 activeFilter 上", () => {
    const r = renderHook(() => useLogsFilters());
    expect(r.result.current.activeFilter.model).toBeUndefined();
    expect(r.result.current.activeFilter.actual_model).toBeUndefined();
    act(() => r.result.current.setFilterModel("claude-3"));
    expect(r.result.current.activeFilter.model).toBe("claude-3");
    expect(r.result.current.activeFilter.actual_model).toBeUndefined();
    act(() => r.result.current.setFilterActualModel("glm-4"));
    expect(r.result.current.activeFilter.model).toBe("claude-3");
    expect(r.result.current.activeFilter.actual_model).toBe("glm-4");
    expect(r.result.current.hasFilter).toBe(true);
  });

  it("两个下拉选项各取自 model / actual_model 列", async () => {
    const r = renderHook(() => useLogsFilters());
    await waitFor(() => expect(r.result.current.modelOptions).toEqual(["claude-3"]));
    expect(r.result.current.actualModelOptions).toEqual(["glm-4"]);
  });

  it("filterStatus success/error 映射为 status=200/-1", () => {
    const r = renderHook(() => useLogsFilters());
    act(() => r.result.current.setFilterStatus("success"));
    expect(r.result.current.activeFilter.status).toBe(200);
    act(() => r.result.current.setFilterStatus("error"));
    expect(r.result.current.activeFilter.status).toBe(-1);
  });

  it("filterObserved=observed 时 activeFilter.observed=true（票 04 观察模式命中筛选）", () => {
    const r = renderHook(() => useLogsFilters());
    expect(r.result.current.activeFilter.observed).toBeUndefined();
    act(() => r.result.current.setFilterObserved("observed"));
    expect(r.result.current.activeFilter.observed).toBe(true);
    expect(r.result.current.hasFilter).toBe(true);
    act(() => r.result.current.setFilterObserved(""));
    expect(r.result.current.activeFilter.observed).toBeUndefined();
  });

  it("clearFilter 把全部筛选字段复位为初始值", () => {
    const r = renderHook(() => useLogsFilters());
    act(() => {
      r.result.current.setFilterPlatform("1");
      r.result.current.setFilterGroup("g1");
      r.result.current.setFilterStatus("error");
      r.result.current.setFilterModel("x");
      r.result.current.setFilterActualModel("y");
      r.result.current.setFilterPath("/v1");
      r.result.current.setFilterObserved("observed");
    });
    expect(r.result.current.hasFilter).toBe(true);
    act(() => r.result.current.clearFilter());
    expect(r.result.current.hasFilter).toBe(false);
    expect(r.result.current.filterPlatform).toBe("");
    expect(r.result.current.filterGroup).toBe("");
    expect(r.result.current.filterStatus).toBe("");
    expect(r.result.current.filterModel).toBe("");
    expect(r.result.current.filterActualModel).toBe("");
    expect(r.result.current.filterPath).toBe("");
    expect(r.result.current.filterObserved).toBe("");
  });

  it("platformMap/groupName 由异步加载的 platforms/groups 派生", async () => {
    const r = renderHook(() => useLogsFilters());
    await waitFor(() => expect(r.result.current.platformMap.get(1)).toBe("P1"));
    expect(r.result.current.groupName("g1")).toBe("G1");
    expect(r.result.current.groupName("unknown")).toBe("unknown");
  });

  // magpie 票 09：会话统计行点击跳日志页，sessionKey 经导航入口带入
  it("初始 sessionKey 带入 activeFilter.session_key，clearFilter 复位", () => {
    const r = renderHook(() => useLogsFilters({ sessionKey: "abc-123" }));
    expect(r.result.current.activeFilter.session_key).toBe("abc-123");
    expect(r.result.current.hasFilter).toBe(true);
    expect(r.result.current.filterSession).toBe("abc-123");
    act(() => r.result.current.clearFilter());
    expect(r.result.current.activeFilter.session_key).toBeUndefined();
    expect(r.result.current.hasFilter).toBe(false);
    expect(r.result.current.filterSession).toBe("");
  });
});

describe("useLogsFilters 类型筛选（隧道连接）", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    platformApiMock.list.mockResolvedValue([]);
    groupDetailApiMock.list.mockResolvedValue([]);
    proxyLogApiMock.listFiltered.mockResolvedValue({ items: [], has_more: false });
    proxyLogApiMock.distinctModels.mockResolvedValue([]);
  });

  it("默认包含隧道连接：不排除 http-connect，无 sources 限定", () => {
    const r = renderHook(() => useLogsFilters());
    expect(r.result.current.activeFilter.exclude_sources).not.toContain("http-connect");
    expect(r.result.current.activeFilter.sources).toBeUndefined();
  });

  it("tunnel = 仅 http-connect", () => {
    const r = renderHook(() => useLogsFilters());
    act(() => r.result.current.setFilterType("tunnel"));
    expect(r.result.current.activeFilter.sources).toEqual(["http-connect"]);
    expect(r.result.current.hasFilter).toBe(true);
  });

  it("request = 排除 http-connect，保留默认排除", () => {
    const r = renderHook(() => useLogsFilters());
    act(() => r.result.current.setFilterType("request"));
    expect(r.result.current.activeFilter.exclude_sources).toEqual(["test", "quota", "fetch-models", "http-connect"]);
  });

  it("clearFilter 复位类型", () => {
    const r = renderHook(() => useLogsFilters());
    act(() => r.result.current.setFilterType("tunnel"));
    act(() => r.result.current.clearFilter());
    expect(r.result.current.activeFilter.sources).toBeUndefined();
  });

  it("prompt_id / session_id 筛选带入 activeFilter（trim），hasFilter 为真，clearFilter 复位", () => {
    const r = renderHook(() => useLogsFilters());
    act(() => { r.result.current.setFilterPromptId(" p-1 "); r.result.current.setFilterSessionId("s-1"); });
    expect(r.result.current.activeFilter.prompt_id).toBe("p-1");
    expect(r.result.current.activeFilter.session_id).toBe("s-1");
    expect(r.result.current.hasFilter).toBe(true);
    act(() => r.result.current.clearFilter());
    expect(r.result.current.activeFilter.prompt_id).toBeUndefined();
    expect(r.result.current.activeFilter.session_id).toBeUndefined();
    expect(r.result.current.hasFilter).toBe(false);
  });
});

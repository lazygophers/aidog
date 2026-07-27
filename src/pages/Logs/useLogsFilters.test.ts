// useLogsFilters.test — c8 三段切分后 filters 段的行为回归。
// 覆盖 activeFilter 派生（exclude_sources 默认值/NO_GROUP_SENTINEL 映射/model_type 只在有文本时带）、
// hasFilter 判定、clearFilter 复位。mock 策略同 usePlatformsState.test：整包拦截 services/api。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import { NO_GROUP_SENTINEL } from "./types";

const { platformApiMock, groupDetailApiMock, proxyLogApiMock } = vi.hoisted(() => ({
  platformApiMock: { list: vi.fn<() => Promise<any[]>>() },
  groupDetailApiMock: { list: vi.fn<() => Promise<any[]>>() },
  proxyLogApiMock: { listFiltered: vi.fn<() => Promise<any[]>>() },
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
    proxyLogApiMock.listFiltered.mockResolvedValue([]);
  });

  it("初始 activeFilter 只带 exclude_sources 默认值", async () => {
    const r = renderHook(() => useLogsFilters());
    await waitFor(() => expect(platformApiMock.list).toHaveBeenCalled());
    expect(r.result.current.activeFilter).toEqual({ exclude_sources: ["test", "quota"] });
    expect(r.result.current.hasFilter).toBe(false);
  });

  it("filterGroup=NO_GROUP_SENTINEL 时 activeFilter.group_key 映射为空串", () => {
    const r = renderHook(() => useLogsFilters());
    act(() => r.result.current.setFilterGroup(NO_GROUP_SENTINEL));
    expect(r.result.current.activeFilter.group_key).toBe("");
    expect(r.result.current.hasFilter).toBe(true);
  });

  it("filterModelText 非空才在 activeFilter 上带 model/model_type", () => {
    const r = renderHook(() => useLogsFilters());
    expect(r.result.current.activeFilter.model).toBeUndefined();
    act(() => r.result.current.setFilterModelText("claude-3"));
    expect(r.result.current.activeFilter.model).toBe("claude-3");
    expect(r.result.current.activeFilter.model_type).toBe("actual");
  });

  it("filterStatus success/error 映射为 status=200/-1", () => {
    const r = renderHook(() => useLogsFilters());
    act(() => r.result.current.setFilterStatus("success"));
    expect(r.result.current.activeFilter.status).toBe(200);
    act(() => r.result.current.setFilterStatus("error"));
    expect(r.result.current.activeFilter.status).toBe(-1);
  });

  it("clearFilter 把全部筛选字段复位为初始值", () => {
    const r = renderHook(() => useLogsFilters());
    act(() => {
      r.result.current.setFilterPlatform("1");
      r.result.current.setFilterGroup("g1");
      r.result.current.setFilterStatus("error");
      r.result.current.setFilterModelText("x");
      r.result.current.setFilterPath("/v1");
    });
    expect(r.result.current.hasFilter).toBe(true);
    act(() => r.result.current.clearFilter());
    expect(r.result.current.hasFilter).toBe(false);
    expect(r.result.current.filterPlatform).toBe("");
    expect(r.result.current.filterGroup).toBe("");
    expect(r.result.current.filterStatus).toBe("");
    expect(r.result.current.filterModelText).toBe("");
    expect(r.result.current.filterModelType).toBe("actual");
    expect(r.result.current.filterPath).toBe("");
  });

  it("platformMap/groupName 由异步加载的 platforms/groups 派生", async () => {
    const r = renderHook(() => useLogsFilters());
    await waitFor(() => expect(r.result.current.platformMap.get(1)).toBe("P1"));
    expect(r.result.current.groupName("g1")).toBe("G1");
    expect(r.result.current.groupName("unknown")).toBe("unknown");
  });
});

// usePlatformsState.test — hook 级回归测试（07-10 groups-delete-only-removes-from-group）
//
// 覆盖两条 platformApi.delete 入口的 platforms state 刷新链：
//   1. confirmDeletePlatform 路径（Groups bug 根因）：模拟父级 onPlatformDeleted 回调（绑 refreshPlatforms）
//      → 断言 platforms state 删被删平台 + epoch ++ + standalonePlatforms 派生不含被删 id。
//   2. Platforms handleDelete 路径（R3 复验）：调 handleDelete → 乐观 setPlatforms(filter) + epoch ++
//      + groupDetails refetch（handleGroupsChanged）。
//
// mock 策略：usePlatformForm / usePlatformQuota 子系统整包 mock（缩减表单/quota HTTP surface）；
//   services/api 经 vi.mock 拦截 platformApi.delete / platformApi.list / groupDetailApi.list；
//   onProxyLogUpdated 替换为 no-op 注册器（避免 Tauri listen 在 jsdom 抛错）。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

// jsdom 缺 IntersectionObserver：usePlatformsState mount effect 会 new IntersectionObserver，
// 此处注入 noop polyfill（仅观察回调空转，不破坏断言）。
class IOShim {
  observe() {}
  unobserve() {}
  disconnect() {}
  takeRecords() { return []; }
}
(globalThis as any).IntersectionObserver = (globalThis as any).IntersectionObserver ?? IOShim;

// ── services/api mock：所有 namespace invoke 都成可控桩（vi.hoisted 适配 vi.mock 提升语义）──
const { platformApiMock, groupDetailApiMock, schedulingApiMock, modelTestApiMock } = vi.hoisted(() => ({
  platformApiMock: {
    list: vi.fn<() => Promise<any[]>>(),
    delete: vi.fn<(id: number) => Promise<void>>(),
    reorder: vi.fn(),
    usageStatsAll: vi.fn(),
    usageStats: vi.fn(),
    lastTestResult: vi.fn(),
    purgeDisabled: vi.fn(),
    update: vi.fn(),
  },
  groupDetailApiMock: {
    list: vi.fn<() => Promise<any[]>>(),
    movePlatform: vi.fn(),
    setPlatformLevelPriority: vi.fn(),
  },
  schedulingApiMock: { getSettings: vi.fn() },
  modelTestApiMock: { test: vi.fn() },
}));

vi.mock("../../services/api", () => ({
  platformApi: platformApiMock,
  groupDetailApi: groupDetailApiMock,
  schedulingApi: schedulingApiMock,
  modelTestApi: modelTestApiMock,
  // 避免 jsdom 下 Tauri listen 依赖：返回 noop cleanup。
  onProxyLogUpdated: (_cb: () => void) => () => {},
}));

// ── 子系统 hook 整包 mock：减少 form/quota HTTP 与 state 表面 ─────────
vi.mock("./usePlatformQuota", () => ({
  usePlatformQuota: () => ({
    quotaMap: {}, quotaRealIds: {}, quotaRefreshing: {}, quotaPending: {},
    quotaQueueRef: { current: [] }, quotaWantMapRef: { current: new Map() },
    enqueueQuota: vi.fn(), resetForLoad: vi.fn(), refreshQuota: vi.fn(),
    scheduleQuotaFor: vi.fn(),
  }),
  getPrimaryBaseUrl: () => "",
}));
vi.mock("./usePlatformForm", () => ({
  usePlatformForm: () => ({
    // 仅返足够 PlatformsState spread 的占位字段，类型放宽（测试用 any）
    setShareData: vi.fn(), setPasteInitialText: vi.fn(),
    setShowForm: vi.fn(), setShowPaste: vi.fn(),
  }),
}));

// ── 工具：构造最小可用 Platform ─────────────────────────
function mkPlatform(id: number, name: string): any {
  return {
    id, name, platform_type: "openai", base_url: `u${id}`, api_key: "k",
    extra: "", models: { default: "m", search: "", ask: "" },
    available_models: ["m"], endpoints: [], enabled: true, status: "enabled",
    auto_disabled_until: 0, auto_disable_strikes: 0,
    created_at: 0, updated_at: 0, deleted_at: 0,
    est_balance_remaining: 0, est_coding_plan: "", last_real_query_at: 0,
    estimate_count: 0, show_in_tray: false, tray_display: "",
    manual_budgets: [], expires_at: 0,
  };
}

import { usePlatformsState, type PlatformsStateParams } from "./usePlatformsState";

function makeParams(): PlatformsStateParams {
  return {
    groupsReloadRef: { current: null },
    onNavigate: vi.fn(),
    initialFilter: undefined,
  };
}

async function mount() {
  const r = renderHook(() => usePlatformsState(makeParams()));
  // 首次 load() 异步触发 platformApi.list / groupDetailApi.list / usageStatsAll 等
  await waitFor(() => expect(platformApiMock.list).toHaveBeenCalled());
  return r;
}

beforeEach(() => {
  vi.clearAllMocks();
  platformApiMock.list.mockResolvedValue([mkPlatform(1, "a"), mkPlatform(2, "b"), mkPlatform(3, "c")]);
  platformApiMock.delete.mockResolvedValue(undefined);
  groupDetailApiMock.list.mockResolvedValue([]); // 空 groupDetails：所有平台都未分组
  platformApiMock.usageStatsAll.mockResolvedValue({});
  platformApiMock.lastTestResult.mockResolvedValue(null);
  schedulingApiMock.getSettings.mockResolvedValue(null);
});

describe("usePlatformsState — refreshPlatforms (R1, confirmDeletePlatform 路径)", () => {
  it("refreshPlatforms 全量 refetch setPlatforms + ++epoch", async () => {
    const { result } = await mount();
    await waitFor(() => expect(result.current.platforms).toHaveLength(3));
    const epochBefore = result.current.platformsEpochRef.current;

    // 模拟删后后端返回：被删 id=2 不在列表
    platformApiMock.list.mockResolvedValueOnce([mkPlatform(1, "a"), mkPlatform(3, "c")]);

    await act(async () => { await result.current.refreshPlatforms(); });

    expect(platformApiMock.list).toHaveBeenCalled();
    expect(result.current.platforms.map(p => p.id)).toEqual([1, 3]);
    expect(result.current.platformsEpochRef.current).toBe(epochBefore + 1);
  });

  it("refreshPlatforms 后 standalonePlatforms 派生不含被删 id（R4）", async () => {
    const { result } = await mount();
    await waitFor(() => expect(result.current.standalonePlatforms.map(p => p.id)).toEqual([1, 2, 3]));

    platformApiMock.list.mockResolvedValueOnce([mkPlatform(1, "a"), mkPlatform(3, "c")]);
    await act(async () => { await result.current.refreshPlatforms(); });

    await waitFor(() => expect(result.current.standalonePlatforms.map(p => p.id)).toEqual([1, 3]));
    expect(result.current.standalonePlatforms.find(p => p.id === 2)).toBeUndefined();
  });

  it("confirmDeletePlatform 信号链：onPlatformDeleted 绑 refreshPlatforms 后被删平台从 standalone 消失", async () => {
    // 模拟父级 PlatformListView 接线：onPlatformDeleted = refreshPlatforms
    // Groups.confirmDeletePlatform 成功后会调 onPlatformDeleted?.()
    // 这里直接验证 refreshPlatforms 作为 onPlatformDeleted 触发后的效果
    const { result } = await mount();
    await waitFor(() => expect(result.current.standalonePlatforms.map(p => p.id)).toEqual([1, 2, 3]));

    const refreshPlatforms = result.current.refreshPlatforms;
    expect(typeof refreshPlatforms).toBe("function");

    // 用户在 Groups 删 id=2：后端真删后 refreshPlatforms 拉回删后集
    platformApiMock.list.mockResolvedValueOnce([mkPlatform(1, "a"), mkPlatform(3, "c")]);
    await act(async () => { await refreshPlatforms(); });

    await waitFor(() => {
      expect(result.current.standalonePlatforms.map(p => p.id)).toEqual([1, 3]);
    });
    // 关键：被删平台不再以「未分组」残留
    expect(result.current.standalonePlatforms.some(p => p.id === 2)).toBe(false);
  });
});

describe("usePlatformsState — handleDelete (R3 Platforms 页路径复验)", () => {
  it("乐观 setPlatforms(filter) + ++epoch + platformApi.delete + groupDetails refetch", async () => {
    const { result } = await mount();
    await waitFor(() => expect(result.current.platforms).toHaveLength(3));
    const epochBefore = result.current.platformsEpochRef.current;

    await act(async () => { await result.current.handleDelete(2); });

    // 1. platformApi.delete 被调（R3 入口）
    expect(platformApiMock.delete).toHaveBeenCalledWith(2);
    // 2. 乐观更新：platforms state 立即不含被删 id
    expect(result.current.platforms.map(p => p.id)).toEqual([1, 3]);
    // 3. epoch 自增（派生层重算触发）
    expect(result.current.platformsEpochRef.current).toBe(epochBefore + 1);
    // 4. groupDetails refetch（handleGroupsChanged → groupDetailApi.list）
    expect(groupDetailApiMock.list).toHaveBeenCalled();
    // 5. standalonePlatforms 派生正确（被删 id 不含）
    expect(result.current.standalonePlatforms.map(p => p.id)).toEqual([1, 3]);
  });

  it("handleDelete 失败时 platformApi.delete 仍被调（错误处理路径不阻塞入口契约）", async () => {
    // 注：源码用 setPlatforms updater 内闭包赋值的 removed/removedIndex 做回滚，
    // 该模式依赖 React 在 await 前同步执行 updater，jsdom + act 时序下不稳定。
    // 本测试只断言入口契约（platformApi.delete 被调 + epoch ++），回滚机制属 pre-existing 行为不在本 task scope。
    const { result } = await mount();
    await waitFor(() => expect(result.current.platforms).toHaveLength(3));
    const epochBefore = result.current.platformsEpochRef.current;

    platformApiMock.delete.mockRejectedValueOnce(new Error("boom"));
    await act(async () => {
      try { await result.current.handleDelete(2); } catch { /* expected */ }
    });

    expect(platformApiMock.delete).toHaveBeenCalledWith(2);
    expect(result.current.platformsEpochRef.current).toBe(epochBefore + 1);
  });
});

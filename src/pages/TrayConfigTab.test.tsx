// 票 I15：托盘配置从二维网格迁移。这里锁的是迁移规则本身 ——
// 取消勾选只翻 enabled，绝不删项；装载段数无上限（票 托盘-颜色分隔符槽位/01）。
// 票 托盘设置-重设计/01：页面骨架 + 实时预览条（C 方向），组件级 vitest 锚 testid/data-state。
import { beforeEach, describe, it, expect, vi } from "vitest";
import { render, waitFor, fireEvent, act } from "../test/render";
import {
  segmentKey,
  segmentOptions,
  toggleSegment,
  moveSegment,
  loadSegmentAt,
gapSeparators,
setGapSeparator,
  previewValue,
  previewLevel,
  segColor,
  trayMoney,
  formatCountdown,
  type SegmentOption,
} from "./TrayConfigTab";
import type { TrayItem, TodayStats, Platform } from "../services/api";
import type { DisplayCurrency } from "../services/api/types";
import { setDisplayCurrency, USD_DISPLAY } from "../utils/displayCurrency";
import { setBalanceCurrencies } from "../utils/balanceCurrency";
import golden from "../../src-tauri/crates/aidog_core/src/testdata/tray_golden.json";
import platformGolden from "../../src-tauri/crates/aidog_core/src/testdata/tray_platform_golden.json";
import levelGolden from "../../src-tauri/crates/aidog_core/src/testdata/tray_level_golden.json";

const mk = (code: string, symbol: string, decimals: number, rate: number): DisplayCurrency =>
  ({ code, symbol, decimals, rate, table: [] });

const t = ((key: string, fallback?: string) => fallback ?? key) as never;

function item(patch: Partial<TrayItem>): TrayItem {
  return {
    item_type: "today_usage", platform_id: null, display: "", metric: "cost", label: null,
    decimals: null, metrics: null, unit: null,
    color: { mode: "follow", value: "" }, font_size: 9, line_mode: "two",
    align: "left", align_row2: "right", enabled: true, order: 0,
    ...patch,
  };
}

const byKey = (k: string): SegmentOption =>
  segmentOptions([], t).find((o) => o.key === k)!;

describe("toggleSegment", () => {
  it("取消勾选只关掉项，不删除，也不动二维字段", () => {
    const items = [item({ metric: "cost" })];
    const next = toggleSegment(items, byKey("today_usage:cost"));
    expect(next).toHaveLength(1);
    expect(next[0].enabled).toBe(false);
    expect(next[0].line_mode).toBe("two");
    expect(next[0].align_row2).toBe("right");
  });

  it("再勾回来复用原项，保留自定义标签", () => {
    const items = [item({ metric: "cost", label: "💰", enabled: false })];
    const next = toggleSegment(items, byKey("today_usage:cost"));
    expect(next).toHaveLength(1);
    expect(next[0].enabled).toBe(true);
    expect(next[0].label).toBe("💰");
  });

  it("已装 3 项仍可再勾，无数量上限", () => {
    const items = [
      item({ metric: "cost", order: 0 }),
      item({ metric: "tokens", order: 1 }),
      item({ item_type: "peak", metric: null, order: 2 }),
    ];
    const next = toggleSegment(items, byKey("routed_platform"));
    expect(next.filter((i) => i.enabled)).toHaveLength(4);
    expect(next).not.toBe(items);
  });

  it("新勾的项追加在末尾，order 按启用顺序重写", () => {
    const items = [item({ metric: "cost", order: 0 })];
    const next = toggleSegment(items, byKey("peak"));
    expect(next.map(segmentKey)).toEqual(["today_usage:cost", "peak"]);
    expect(next.map((i) => i.order)).toEqual([0, 1]);
  });

  it("关掉的项被排到启用项之后", () => {
    const items = [item({ metric: "cost", order: 0 }), item({ metric: "tokens", order: 1 })];
    const next = toggleSegment(items, byKey("today_usage:cost"));
    expect(next.map(segmentKey)).toEqual(["today_usage:tokens", "today_usage:cost"]);
    expect(next[0].enabled).toBe(true);
    expect(next[1].enabled).toBe(false);
  });
});

describe("previewValue", () => {
  it("今日费用取 today_stats.cost（与统计页同一份数据）", () => {
    const stats = { tokens: 5, cache_rate: 0, cost: 1.25, total_requests: 2 } as TodayStats;
    expect(previewValue(item({ metric: "cost" }), [], stats)).toBe("$1.25");
  });

  it("命中平台仍占位；peak 段与真实托盘同口径：高峰「峰」，其余「平」", () => {
    expect(previewValue(item({ item_type: "routed_platform", metric: null }), [], null)).toBe("—");
    const pk = item({ item_type: "peak", metric: null });
    expect(previewValue(pk, [], null)).toBe("平");
    expect(previewValue(pk, [], null, { platform_name: "A", in_peak: false, multiplier: 1 })).toBe("平");
    expect(previewValue(pk, [], null, { platform_name: "A", in_peak: true, multiplier: 2 })).toBe("峰");
  });
});

// 跨层 golden：与 Rust test_tray_render.rs 的 tray_money_golden_cases_match 共读同一份 tray_golden.json。
describe("trayMoney（对拍 Rust tray_money）", () => {
  it.each(golden as Array<{ code: string; symbol: string; decimals: number; rate: number; usd: number; d: number; unit?: string; pos?: string | null; expected: string }>)(
    "$code $usd $unit $pos -> $expected",
    (c) => {
      expect(trayMoney(c.usd, mk(c.code, c.symbol, c.decimals, c.rate), c.d, c.unit, c.pos)).toBe(c.expected);
    },
  );

  it("previewValue：今日花费 cost 段 unit / unit_position 与 Rust 同口径", () => {
    const stats = { tokens: 0, cache_rate: 0, cost: 0.242857142857, total_requests: 0 } as TodayStats;
    const cny = mk("CNY", "¥", 2, 7.0);
    const p = (patch: Partial<TrayItem>) => previewValue(item({ metric: "cost", decimals: 2, ...patch }), [], stats, null, cny);
    expect(p({})).toBe("¥1.7");
    expect(p({ unit: "元" })).toBe("1.7元");
    expect(p({ unit: "元", unit_position: "prefix" })).toBe("元1.7");
    expect(p({ unit_position: "suffix" })).toBe("1.7¥");
  });

  it("previewValue：花费 / 平台余额按显示币种，与 trayMoney 同口径", () => {
    const stats = { tokens: 0, cache_rate: 0, cost: 0.0123456, total_requests: 0 } as TodayStats;
    const cny = mk("CNY", "¥", 2, 7.0);
    expect(previewValue(item({ metric: "cost" }), [], stats, null, cny)).toBe("¥0.08642");
    const p = { id: 7, est_balance_remaining: 10 } as Platform;
    expect(previewValue(item({ item_type: "platform", platform_id: 7, metric: null }), [p], null, null, mk("JPY", "JP¥", 0, 150))).toBe("JP¥1500");
  });

  it("previewValue：CNY 余额先折 USD 再按显示币种（对拍 Rust test_tray_render CNY 余额例）", () => {
    setDisplayCurrency({ ...USD_DISPLAY, table: [{ code: "CNY", symbol: "¥", decimals: 2, default_rate: 6.6675, rate: 6.6675 }] } as never);
    setBalanceCurrencies({ "7": "CNY" });
    const p = { id: 7, est_balance_remaining: 66.675 } as Platform;
    const pi = item({ item_type: "platform", platform_id: 7, metric: null });
    expect(previewValue(pi, [p], null, null, USD_DISPLAY)).toBe("$10"); // 对拍 bal(Some("CNY"), usd())
    expect(previewValue(pi, [p], null, null, mk("CNY", "¥", 2, 6.6675))).toBe("¥66.67"); // 对拍 CNY 显示恒等
    setBalanceCurrencies({});
    setDisplayCurrency(USD_DISPLAY);
  });
});

// ── 平台槽多指标（metrics / unit / decimals / coding 窗口 / reset_time）──

const plat = {
  id: 7,
  name: "Acme",
  enabled: true,
  est_balance_remaining: 10.1,
  est_coding_plan: JSON.stringify({
    tiers: [
      { name: "five_hour", est_utilization: 25, window_start: 0 },
      { name: "weekly_limit", est_utilization: 50 },
    ],
  }),
} as never;

function platformItem(patch: Partial<TrayItem>): TrayItem {
  return item({ item_type: "platform", platform_id: 7, metric: null, ...patch });
}

describe("previewValue 平台槽多指标", () => {
  const stats = { tokens: 1240, cache_rate: 0, cost: 0.5, total_requests: 3 } as TodayStats;

  it("多指标按词表序以 ' · ' 拼进同一格", () => {
    expect(
      previewValue(
        platformItem({ metrics: ["balance", "today_cost", "today_tokens"] }),
        [plat],
        stats,
      ),
    ).toBe("$10.1 · $0.5 · 1240 tok");
  });

  it("unit 替换默认单位（$ 前缀去掉、后缀接管）；decimals 上限裁尾零", () => {
    expect(
      previewValue(
        platformItem({ metrics: ["balance", "today_tokens"], unit: " U", decimals: 3 }),
        [plat],
        stats,
      ),
    ).toBe("10.1 U · 1240 U");
  });

  // 跨层 golden：与 Rust platform_metrics_use_display_currency 共读 tray_platform_golden.json。
  // reset 用例（带 now_local_h / ws_local_h）的 now 与 window_start 取「本地零点 + 小时偏移」，
  // 与 Rust 侧同式推导：同一固定 now 注入（防秒边界抖动），绝对时间不随时区漂移。
  it.each(platformGolden as Array<{ code: string; symbol: string; decimals: number; rate: number; balance: number; balance_ccy: string | null; cost: number; metrics: string[]; unit: string | null; ws_local_h?: number; now_local_h?: number; reset_format?: string; expected: string }>)(
    "多指标金额按显示币种（对拍 Rust）$expected",
    (c) => {
      const cur = mk(c.code, c.symbol, c.decimals, c.rate);
      setDisplayCurrency({ ...USD_DISPLAY, table: [{ code: c.code, symbol: c.symbol, decimals: c.decimals, default_rate: c.rate, rate: c.rate }] } as never);
      setBalanceCurrencies(c.balance_ccy ? { "7": c.balance_ccy } : {});
      const midnight = (() => { const d = new Date(); d.setHours(0, 0, 0, 0); return d.getTime(); })();
      const hasReset = c.ws_local_h != null && c.now_local_h != null;
      const p = {
        ...(plat as object),
        est_balance_remaining: c.balance,
        ...(hasReset
          ? { est_coding_plan: JSON.stringify({ tiers: [{ name: "five_hour", est_utilization: 25, window_start: midnight + c.ws_local_h! * 3_600_000 }] }) }
          : {}),
      } as Platform;
      const st = { tokens: 0, cache_rate: 0, cost: c.cost, total_requests: 0 } as TodayStats;
      const it = platformItem({ metrics: c.metrics, ...(c.unit ? { unit: c.unit } : {}), ...(c.reset_format ? { reset_format: c.reset_format } : {}) });
      expect(previewValue(it, [p], st, null, cur, hasReset ? midnight + c.now_local_h! * 3_600_000 : undefined)).toBe(c.expected);
      setBalanceCurrencies({});
      setDisplayCurrency(USD_DISPLAY);
    },
  );

  it("coding 窗口取指定 tier 剩余%；reset_time 窗口无起点/未知 cycle 跳过", () => {
    expect(
      previewValue(
        platformItem({ metrics: ["coding:weekly_limit", "coding:five_hour", "reset_time:five_hour"] }),
        [plat],
        stats,
      ),
      // weekly=50%、five=75%；five_hour window_start=0 → reset 跳过
    ).toBe("50% · 75%");
  });
});

// ── 票 01：页面骨架 + 实时预览条 ──

const api = vi.hoisted(() => ({
  list: vi.fn(),
  get: vi.fn(),
  todayStats: vi.fn(),
  set: vi.fn(),
  onProxyLogUpdated: vi.fn(),
  peakStatus: vi.fn(),
  statsQuery: vi.fn(),
}));
vi.mock("../services/api", () => ({
  platformApi: { list: api.list },
  trayConfigApi: { get: api.get, set: api.set, todayStats: api.todayStats, peakStatus: api.peakStatus },
  statsApi: { query: api.statsQuery },
  onProxyLogUpdated: api.onProxyLogUpdated,
}));

const stats = { tokens: 1240000, input_tokens: 0, output_tokens: 0, cache_tokens: 0, cache_rate: 74, cost: 1.25, total_requests: 86 };

async function renderTab(
  cfg?: { separator: string; items: TrayItem[] },
  platforms: unknown[] = [],
  peak: { platform_name: string; in_peak: boolean; multiplier: number } | null = null,
  trend: { buckets: unknown[]; series: unknown[] } = { buckets: [], series: [] },
) {
  const { TrayConfigTab } = await import("./TrayConfigTab");
  api.list.mockResolvedValue(platforms);
  api.get.mockResolvedValue(cfg ?? {
    separator: "  ",
    items: [
      item({ metric: "cost", order: 0 }),
      item({ metric: "tokens", order: 1 }),
    ],
  });
  api.todayStats.mockResolvedValue(stats);
  api.peakStatus.mockResolvedValue(peak);
  api.statsQuery.mockResolvedValue(trend);
  api.onProxyLogUpdated.mockReturnValue(() => {});
  const view = render(<TrayConfigTab />);
  await waitFor(() => expect(view.getByTestId("tray-menubar")).toBeTruthy());
  return view;
}

describe("TrayConfigTab 骨架与预览（票 01）", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("切显示币种后预览段与今日消耗大字即时换算（托盘口径）", async () => {
    const view = await renderTab();
    expect(view.getByTestId("tray-segment-today_usage:cost").textContent).toContain("$1.25");
    act(() => setDisplayCurrency(mk("CNY", "¥", 2, 7.0)));
    await waitFor(() => expect(view.getByTestId("tray-segment-today_usage:cost").textContent).toContain("¥8.75"));
    expect(view.getByTestId("tray-big-cost").textContent).toBe("¥8.75");
    act(() => setDisplayCurrency(USD_DISPLAY));
  });

  it("预览条按装载顺序渲染段，数值随 todayStats 实时刷新", async () => {
    const view = await renderTab();
    const bar = view.getByTestId("tray-menubar");
    expect(bar.getAttribute("style")).toContain("var(--mb-bg)");
    expect(view.getByTestId("tray-segment-today_usage:cost").textContent).toContain("$1.25");
    expect(view.getByTestId("tray-segment-today_usage:tokens").textContent).toContain("1240000 tok");
  });

  it("页头计数反映已装载段数，不再有 /N 上限", async () => {
    const view = await renderTab();
    expect(view.getByTestId("tray-count").textContent).toContain("2");
    expect(view.getByTestId("tray-count").textContent).not.toContain("/");
  });

  it("装载槽 = 已装 N 段 + 1 个尾部空槽：已装载槽显示段值，空槽标 empty", async () => {
    const view = await renderTab();
    expect(view.getByTestId("tray-slot-0").getAttribute("data-state")).toBe("filled");
    expect(view.getByTestId("tray-slot-0").textContent).toContain("$1.25");
    expect(view.getByTestId("tray-slot-1").getAttribute("data-state")).toBe("filled");
    expect(view.getByTestId("tray-slot-2").getAttribute("data-state")).toBe("empty");
    expect(view.queryByTestId("tray-slot-3")).toBeNull();
  });

  it("装入一段后槽数 +1（始终保留尾部空槽）", async () => {
    const view = await renderTab();
    fireEvent.click(view.getByTestId("tray-chip-peak"));
    expect(view.getByTestId("tray-slot-2").getAttribute("data-state")).toBe("filled");
    expect(view.getByTestId("tray-slot-3").getAttribute("data-state")).toBe("empty");
    expect(view.queryByTestId("tray-slot-4")).toBeNull();
  });

  it("移除一段后槽数 -1", async () => {
    const view = await renderTab();
    fireEvent.click(view.getByTestId("tray-chip-today_usage:cost"));
    expect(view.getByTestId("tray-slot-0").getAttribute("data-state")).toBe("filled");
    expect(view.getByTestId("tray-slot-1").getAttribute("data-state")).toBe("empty");
    expect(view.queryByTestId("tray-slot-2")).toBeNull();
  });

  it("已装 6 段渲染 7 个槽", async () => {
    const view = await renderTab({
      separator: "  ",
      items: [
        item({ metric: "cost", order: 0 }),
        item({ metric: "tokens", order: 1 }),
        item({ metric: "requests", order: 2 }),
        item({ metric: "cache_rate", order: 3 }),
        item({ item_type: "peak", metric: null, order: 4 }),
        item({ item_type: "routed_platform", metric: null, order: 5 }),
      ],
    });
    expect(view.getByTestId("tray-slot-5").getAttribute("data-state")).toBe("filled");
    expect(view.getByTestId("tray-slot-6").getAttribute("data-state")).toBe("empty");
    expect(view.queryByTestId("tray-slot-7")).toBeNull();
  });

  it("候选库芯片带 loaded/空 data-state（装载交互见票 02 组）", async () => {
    const view = await renderTab();
    expect(view.getByTestId("tray-chip-today_usage:cost").getAttribute("data-state")).toBe("loaded");
    expect(view.getByTestId("tray-chip-today_usage:tokens").getAttribute("data-state")).toBe("loaded");
    expect(view.getByTestId("tray-chip-peak").getAttribute("data-state")).toBe("");
  });

  it("候选库平台行含全部平台（含禁用 / 自动禁用），不按 enabled 过滤", async () => {
    const mkP = (id: number, status: string) => ({ id, name: `P${id}`, enabled: status === "enabled", status });
    const view = await renderTab(undefined, [mkP(1, "enabled"), mkP(2, "disabled"), mkP(3, "auto_disabled")]);
    for (const id of [1, 2, 3]) expect(view.getByTestId(`tray-chip-platform:${id}`)).toBeTruthy();
  });

  it("预览条颜色走主题 token，页内无字面色（禁硬编码色）", async () => {
    const view = await renderTab();
    const bar = view.getByTestId("tray-menubar").getAttribute("style") ?? "";
    for (const tok of ["var(--mb-bg)", "var(--mb-border)", "var(--mb-fg)"]) {
      expect(bar).toContain(tok);
    }
    const seg = view.getByTestId("tray-segment-today_usage:cost");
    expect(seg.innerHTML).toContain("var(--mb-fg-strong)");
    // 整页渲染树不出现字面色值
    expect(view.container.innerHTML).not.toMatch(/#[0-9a-fA-F]{3,8}\b/);
    expect(view.container.innerHTML).not.toMatch(/rgba?\(/);
  });

  it("双主题：globals.css 的 light 缺省与 dark 覆盖各自定义 --mb-*，light 不是 dark 的附庸", async () => {
    const { readFileSync } = await import("node:fs");
    const { resolve } = await import("node:path");
    const css = readFileSync(resolve(process.cwd(), "src/styles/globals.css"), "utf8");
    const rootBlock = css.slice(css.indexOf(":root {"), css.indexOf("html,"));
    const darkBlock = css.slice(css.indexOf(':root[data-mode="dark"] {'));
    for (const v of ["--mb-bg", "--mb-border", "--mb-fg", "--mb-fg-dim", "--mb-fg-strong"]) {
      expect(rootBlock).toContain(`${v}:`);
      expect(darkBlock).toContain(`${v}:`);
    }
    const lightBg = rootBlock.match(/--mb-bg: ([^;]+);/)![1];
    const darkBg = darkBlock.match(/--mb-bg: ([^;]+);/)![1];
    expect(lightBg).not.toBe(darkBg);
  });
});

describe("高峰徽标与近 7 日趋势（预览区真数据）", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });
  const peakCfg = { separator: "  ", items: [item({ item_type: "peak", metric: null, order: 0 })] };

  it("最近命中平台在高峰 → 徽标显示平台名与倍率，预览段为「峰」", async () => {
    const view = await renderTab(peakCfg, [], { platform_name: "Acme", in_peak: true, multiplier: 2 });
    await waitFor(() => expect(view.getByTestId("tray-peak-badge").textContent).toContain("Acme ×2"));
    expect(view.getByTestId("tray-segment-peak").textContent).toContain("峰");
  });

  it("非高峰 → 徽标隐藏，预览段为「平」", async () => {
    const view = await renderTab(peakCfg, [], { platform_name: "Acme", in_peak: false, multiplier: 1 });
    await waitFor(() => expect(api.peakStatus).toHaveBeenCalled());
    expect(view.queryByTestId("tray-peak-badge")).toBeNull();
    expect(view.getByTestId("tray-segment-peak").textContent).toContain("平");
  });

  it("无最近命中平台 → 徽标隐藏", async () => {
    const view = await renderTab(peakCfg, [], null);
    await waitFor(() => expect(api.peakStatus).toHaveBeenCalled());
    expect(view.queryByTestId("tray-peak-badge")).toBeNull();
  });

  it("趋势卡按 statsApi.query 的全局 7 日按天查询渲染", async () => {
    const view = await renderTab(undefined, [], null, {
      buckets: [{ time_bucket: "2026-10-28", total_cost: 1.5 }, { time_bucket: "2026-10-29", total_cost: 2.5 }],
      series: [],
    });
    await waitFor(() => expect(api.statsQuery).toHaveBeenCalled());
    const q = api.statsQuery.mock.calls[0][0];
    expect(q.granularity).toBe("daily");
    expect(q.filter_group).toBeUndefined();
    expect(q.filter_platform).toBeUndefined();
    expect(Math.round((q.end - q.start) / 86_400_000)).toBe(7);
    expect(view.getByTestId("tray-trend")).toBeTruthy();
    await waitFor(() => expect(view.getByTestId("tray-trend").querySelector("svg")).toBeTruthy());
  });
});

describe("候选库装载（票 02）", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("点未装载芯片 → 装载进槽并即时持久化（tray_config_set 调用）", async () => {
    const view = await renderTab();
    expect(view.getByTestId("tray-chip-peak").getAttribute("data-state")).toBe("");
    fireEvent.click(view.getByTestId("tray-chip-peak"));
    expect(view.getByTestId("tray-chip-peak").getAttribute("data-state")).toBe("loaded");
    expect(view.getByTestId("tray-slot-2").getAttribute("data-state")).toBe("filled");
    expect(api.set).toHaveBeenCalledTimes(1);
    const sent = api.set.mock.calls[0][0];
    expect(sent.items.filter((i: TrayItem) => i.enabled)).toHaveLength(3);
  });

  it("点已装载芯片 → 卸载只翻 enabled 不删项", async () => {
    const view = await renderTab();
    fireEvent.click(view.getByTestId("tray-chip-today_usage:cost"));
    expect(view.getByTestId("tray-chip-today_usage:cost").getAttribute("data-state")).not.toBe("loaded");
    const sent = api.set.mock.calls[0][0];
    // 原项仍在 items 里，只是 enabled=false
    const kept = sent.items.find((i: TrayItem) => segmentKey(i) === "today_usage:cost");
    expect(kept).toBeTruthy();
    expect(kept.enabled).toBe(false);
  });

  it("已装 3 段：未选项仍可装载（无 locked）", async () => {
    const view = await renderTab({
      separator: "  ",
      items: [
        item({ metric: "cost", order: 0 }),
        item({ metric: "tokens", order: 1 }),
        item({ item_type: "peak", metric: null, order: 2 }),
      ],
    });
    const chip = view.getByTestId("tray-chip-routed_platform");
    expect(chip.getAttribute("data-state")).toBe("");
    fireEvent.click(chip);
    expect(api.set.mock.calls[0][0].items.filter((i: TrayItem) => i.enabled)).toHaveLength(4);
  });

  it("persist 失败 → 错误态提示出现", async () => {
    api.set.mockRejectedValueOnce(new Error("boom"));
    const view = await renderTab();
    fireEvent.click(view.getByTestId("tray-chip-peak"));
    // 测试 i18n 缺译文回退 key 本身（无插值），断言提示出现即可
    await waitFor(() => expect(view.getByTestId("tray-error")).toBeTruthy());
  });

  it("keptDisabled：不在候选清单的旧配置项保留并有提示", async () => {
    const view = await renderTab({
      separator: "  ",
      items: [
        item({ metric: "cost", order: 0 }),
        // 平台清单为空 → platform:999 不在候选里，视为迁移保留项
        item({ item_type: "platform", platform_id: 999, metric: null, enabled: false, order: 1 }),
      ],
    });
    expect(view.getByTestId("tray-count").textContent).toContain("1");
    // 提示在（测试 i18n 回退 key，不插值，只断元素存在）
    expect(view.getByTestId("tray-kept-disabled")).toBeTruthy();
  });

  it("chip 键盘可达：Enter 装载进槽", async () => {
    const view = await renderTab();
    fireEvent.keyDown(view.getByTestId("tray-chip-peak"), { key: "Enter" });
    expect(view.getByTestId("tray-slot-2").getAttribute("data-state")).toBe("filled");
  });
});

// ── 票 03：段编辑面板 —— TrayItem 字段全量露出 ──

describe("段编辑面板（票 03）", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  async function openEditor() {
    const view = await renderTab({
      separator: "  ",
      items: [item({ metric: "cost", line_mode: "single", align_row2: null, order: 0 })],
    });
    fireEvent.click(view.getByTestId("tray-slot-0"));
    await waitFor(() => expect(view.getByTestId("tray-editor")).toBeTruthy());
    expect(view.getByTestId("tray-editor").getAttribute("data-editing")).toBe("today_usage:cost");
    return view;
  }

  it("平台槽指标多选：勾选写 metrics 持久化，全不勾回落 null；unit 输入持久化", async () => {
    const view = await renderTab(
      {
        separator: "  ",
        items: [
          item({ item_type: "platform", platform_id: 7, metric: null, line_mode: "single", align_row2: null, order: 0 }),
        ],
      },
      [plat],
    );
    fireEvent.click(view.getByTestId("tray-slot-0"));
    await waitFor(() => expect(view.getByTestId("tray-editor")).toBeTruthy());
    // 平台槽编辑器露出：指标项（含 coding 窗口与 reset_time 窗口）+ 单位输入
    expect(view.getByTestId("tray-edit-metric-balance")).toBeTruthy();
    expect(view.getByTestId("tray-edit-metric-coding:five_hour")).toBeTruthy();
    expect(view.getByTestId("tray-edit-metric-reset_time:five_hour")).toBeTruthy();
    expect(view.getByTestId("tray-edit-unit")).toBeTruthy();
    // 勾 balance → metrics=[balance]
    fireEvent.click(view.getByTestId("tray-edit-metric-balance"));
    let sent = api.set.mock.calls[0][0];
    const seg0 = sent.items.find((i: TrayItem) => segmentKey(i) === "platform:7");
    expect(seg0.metrics).toEqual(["balance"]);
    // 再勾 reset_time:five_hour → 追加在其后（词表序）
    fireEvent.click(view.getByTestId("tray-edit-metric-reset_time:five_hour"));
    sent = api.set.mock.calls[1][0];
    expect(sent.items.find((i: TrayItem) => segmentKey(i) === "platform:7").metrics).toEqual([
      "balance",
      "reset_time:five_hour",
    ]);
    // 取消全部 → metrics=null 回落旧口径
    fireEvent.click(view.getByTestId("tray-edit-metric-balance"));
    fireEvent.click(view.getByTestId("tray-edit-metric-reset_time:five_hour"));
    sent = api.set.mock.calls[3][0];
    expect(sent.items.find((i: TrayItem) => segmentKey(i) === "platform:7").metrics).toBeNull();
    // unit 输入持久化（空串归 null）
    fireEvent.change(view.getByTestId("tray-edit-unit"), { target: { value: " U" } });
    sent = api.set.mock.calls[4][0];
    expect(sent.items.find((i: TrayItem) => segmentKey(i) === "platform:7").unit).toBe(" U");
    fireEvent.change(view.getByTestId("tray-edit-unit"), { target: { value: "" } });
    sent = api.set.mock.calls[5][0];
    expect(sent.items.find((i: TrayItem) => segmentKey(i) === "platform:7").unit).toBeNull();
  });

  it("重置剩余时间（reset_time）与重置时刻（reset_at）是两个独立勾选项；格式项仅在勾选剩余时间后露出，写 reset_format", async () => {
    const view = await renderTab(
      {
        separator: "  ",
        items: [
          item({ item_type: "platform", platform_id: 7, metric: null, line_mode: "single", align_row2: null, order: 0 }),
        ],
      },
      [plat],
    );
    fireEvent.click(view.getByTestId("tray-slot-0"));
    await waitFor(() => expect(view.getByTestId("tray-editor")).toBeTruthy());
    expect(view.getByTestId("tray-edit-metric-reset_time:five_hour")).toBeTruthy();
    expect(view.getByTestId("tray-edit-metric-reset_at:five_hour")).toBeTruthy();
    expect(view.queryByTestId("tray-edit-metric-reset_in:five_hour")).toBeNull();
    expect(view.queryByTestId("tray-edit-reset-format")).toBeNull();
    // 仅勾重置时刻：不露出倒计时格式项。
    fireEvent.click(view.getByTestId("tray-edit-metric-reset_at:five_hour"));
    expect(view.queryByTestId("tray-edit-reset-format")).toBeNull();
    fireEvent.click(view.getByTestId("tray-edit-metric-reset_at:five_hour"));
    fireEvent.click(view.getByTestId("tray-edit-metric-reset_time:five_hour"));
    const sel = view.getByTestId("tray-edit-reset-format") as HTMLSelectElement;
    expect(sel.value).toBe("cn");
    fireEvent.change(sel, { target: { value: "clock" } });
    const sent = api.set.mock.calls.at(-1)![0];
    expect(sent.items.find((i: TrayItem) => segmentKey(i) === "platform:7").reset_format).toBe("clock");
    expect(sent.items.find((i: TrayItem) => segmentKey(i) === "platform:7").metrics).toEqual(["reset_time:five_hour"]);
    // 再勾重置时刻：两项并存，格式项仍在。
    fireEvent.click(view.getByTestId("tray-edit-metric-reset_at:five_hour"));
    expect(api.set.mock.calls.at(-1)![0].items.find((i: TrayItem) => segmentKey(i) === "platform:7").metrics)
      .toEqual(["reset_time:five_hour", "reset_at:five_hour"]);
    expect(view.getByTestId("tray-edit-reset-format")).toBeTruthy();
  });

  it("单位（票 04）：今日花费 cost 段露出 unit 与位置并持久化；非金额段两项位置都不露出", async () => {
    const view = await openEditor();
    expect(view.getByTestId("tray-edit-unit")).toBeTruthy();
    fireEvent.click(view.getByTestId("tray-edit-unitpos-suffix"));
    let sent = api.set.mock.calls[0][0];
    expect(sent.items[0].unit_position).toBe("suffix");
    expect(view.getByTestId("tray-edit-unitpos-suffix").getAttribute("data-state")).toBe("active");
    fireEvent.click(view.getByTestId("tray-edit-unitpos-prefix"));
    sent = api.set.mock.calls[1][0];
    expect(sent.items[0].unit_position).toBe("prefix");
    fireEvent.change(view.getByTestId("tray-edit-unit"), { target: { value: "元" } });
    sent = api.set.mock.calls[2][0];
    expect(sent.items[0].unit).toBe("元");
  });

  it("单位（票 04）：requests 段不露出位置项", async () => {
    const view = await renderTab({
      separator: "  ",
      items: [item({ metric: "requests", line_mode: "single", align_row2: null, order: 0 })],
    });
    fireEvent.click(view.getByTestId("tray-slot-0"));
    await waitFor(() => expect(view.getByTestId("tray-editor")).toBeTruthy());
    expect(view.queryByTestId("tray-edit-unitpos-suffix")).toBeNull();
    expect(view.queryByTestId("tray-edit-unit")).toBeNull();
  });

  it("点已装载槽打开编辑面板，露出全部七类字段控件", async () => {
    const view = await openEditor();
    expect(view.getByTestId("tray-edit-label")).toBeTruthy();
    expect(view.getByTestId("tray-edit-decimals")).toBeTruthy();
    expect(view.getByTestId("tray-edit-color-follow")).toBeTruthy();
    expect(view.getByTestId("tray-edit-color-preset")).toBeTruthy();
    expect(view.getByTestId("tray-edit-color-custom")).toBeTruthy();
    expect(view.getByTestId("tray-edit-fontsize")).toBeTruthy();
    expect(view.getByTestId("tray-edit-line-two")).toBeTruthy();
    expect(view.getByTestId("tray-edit-align-left")).toBeTruthy();
    // 单行模式下 align_row2 不露出
    expect(view.queryByTestId("tray-edit-align2-left")).toBeNull();
  });

  it("label：编辑即时联动预览并持久化", async () => {
    const view = await openEditor();
    fireEvent.change(view.getByTestId("tray-edit-label"), { target: { value: "花费" } });
    expect(view.getByTestId("tray-segment-today_usage:cost").textContent).toContain("花费");
    const sent = api.set.mock.calls[0][0];
    expect(sent.items.find((i: TrayItem) => segmentKey(i) === "today_usage:cost").label).toBe("花费");
  });

  it("decimals：改小数位即时反映到预览值并持久化", async () => {
    const view = await openEditor();
    fireEvent.change(view.getByTestId("tray-edit-decimals"), { target: { value: "0" } });
    expect(view.getByTestId("tray-segment-today_usage:cost").textContent).toContain("$1");
    const sent = api.set.mock.calls[0][0];
    expect(sent.items.find((i: TrayItem) => segmentKey(i) === "today_usage:cost").decimals).toBe(0);
  });

  it("color preset：选红色 → 预览值染预设色并持久化", async () => {
    const view = await openEditor();
    fireEvent.click(view.getByTestId("tray-edit-color-preset"));
    fireEvent.click(view.getByTestId("tray-edit-preset-red"));
    const seg = view.getByTestId("tray-segment-today_usage:cost").innerHTML;
    expect(seg).toContain("var(--mb-red)");
    const sent = api.set.mock.calls[1][0];
    const it = sent.items.find((i: TrayItem) => segmentKey(i) === "today_usage:cost");
    expect(it.color).toEqual({ mode: "preset", value: "red" });
  });

  it("color custom：合法 hex 落库并上预览，非法 hex 拒绝且不持久化", async () => {
    const view = await openEditor();
    fireEvent.click(view.getByTestId("tray-edit-color-custom"));
    fireEvent.change(view.getByTestId("tray-edit-hex"), { target: { value: "#A1B2C3" } });
    let sent = api.set.mock.calls[0][0];
    let it = sent.items.find((i: TrayItem) => segmentKey(i) === "today_usage:cost");
    expect(it.color).toEqual({ mode: "custom", value: "#A1B2C3" });
    // jsdom 把 hex 归一化成 rgb 序列化，取 style.color 比对（#A1B2C3 → rgb(161,178,195)）
    expect(view.getByTestId("tray-segment-today_usage:cost").querySelector("span:last-child")?.getAttribute("style")).toContain("color");

    // 非法值：提示出现、不落库
    fireEvent.change(view.getByTestId("tray-edit-hex"), { target: { value: "red" } });
    expect(view.getByTestId("tray-edit-hex-invalid")).toBeTruthy();
    const callCount = api.set.mock.calls.length;
    const last = api.set.mock.calls[callCount - 1][0];
    const lastIt = last.items.find((i: TrayItem) => segmentKey(i) === "today_usage:cost");
    expect(lastIt.color.value).toBe("#A1B2C3");
  });

  it("font_size：改字号持久化", async () => {
    const view = await openEditor();
    fireEvent.change(view.getByTestId("tray-edit-fontsize"), { target: { value: "13" } });
    const sent = api.set.mock.calls[0][0];
    expect(sent.items.find((i: TrayItem) => segmentKey(i) === "today_usage:cost").font_size).toBe(13);
  });

  it("line_mode：切两行 → 预览变两行布局、align_row2 露出并持久化", async () => {
    const view = await openEditor();
    fireEvent.click(view.getByTestId("tray-edit-line-two"));
    const seg = view.getByTestId("tray-segment-today_usage:cost");
    expect(seg.getAttribute("style")).toContain("column");
    expect(view.getByTestId("tray-edit-align2-right")).toBeTruthy();
    const sent = api.set.mock.calls[0][0];
    expect(sent.items.find((i: TrayItem) => segmentKey(i) === "today_usage:cost").line_mode).toBe("two");
  });

  it("align：改对齐持久化；两行模式下 align_row2 可单独设置", async () => {
    const view = await openEditor();
    fireEvent.click(view.getByTestId("tray-edit-align-center"));
    let sent = api.set.mock.calls[0][0];
    expect(sent.items.find((i: TrayItem) => segmentKey(i) === "today_usage:cost").align).toBe("center");

    fireEvent.click(view.getByTestId("tray-edit-line-two"));
    fireEvent.click(view.getByTestId("tray-edit-align2-right"));
    sent = api.set.mock.calls[api.set.mock.calls.length - 1][0];
    const it = sent.items.find((i: TrayItem) => segmentKey(i) === "today_usage:cost");
    expect(it.align_row2).toBe("right");
    expect(it.align).toBe("center");
  });

  it("切段重挂：A 段非法 hex 展开态切到 B 段后清空（key=editingKey）", async () => {
    const view = await renderTab({
      separator: "  ",
      items: [item({ metric: "cost", order: 0 }), item({ metric: "tokens", order: 1 })],
    });
    fireEvent.click(view.getByTestId("tray-slot-0"));
    await waitFor(() => expect(view.getByTestId("tray-editor")).toBeTruthy());
    fireEvent.click(view.getByTestId("tray-edit-color-custom"));
    fireEvent.change(view.getByTestId("tray-edit-hex"), { target: { value: "zzz" } });
    expect(view.getByTestId("tray-edit-hex-invalid")).toBeTruthy();
    fireEvent.click(view.getByTestId("tray-slot-1"));
    expect(view.getByTestId("tray-editor").getAttribute("data-editing")).toBe("today_usage:tokens");
    expect(view.queryByTestId("tray-edit-hex")).toBeNull();
    expect(view.queryByTestId("tray-edit-hex-invalid")).toBeNull();
  });
});

// ── 拖拽：候选库拖入装载槽 + 槽内排序 ──
// jsdom 无布局，不模拟指针；截获 DndContext 的 onDragEnd 直接喂拖拽结果，锁住「落点 → 持久化」。
const dnd = vi.hoisted(() => ({ onDragEnd: null as null | ((e: unknown) => void) }));
vi.mock("@dnd-kit/core", async (orig) => {
  const actual = await orig<typeof import("@dnd-kit/core")>();
  return {
    ...actual,
    DndContext: (props: { onDragEnd: (e: unknown) => void } & Record<string, unknown>) => {
      dnd.onDragEnd = props.onDragEnd;
      return <actual.DndContext {...(props as never)} />;
    },
  };
});

const drop = (active: string, over: string | null) =>
  dnd.onDragEnd!({ active: { id: active }, over: over ? { id: over } : null });
const sentKeys = (n = 0) =>
  (api.set.mock.calls[n][0].items as TrayItem[]).filter((i) => i.enabled).map(segmentKey);

describe("moveSegment / loadSegmentAt", () => {
  const three = [
    item({ metric: "cost", order: 0 }), item({ metric: "tokens", order: 1 }),
    item({ item_type: "peak", metric: null, order: 2 }),
  ];
  it("moveSegment：启用序列内换位，order 重写，未启用项留在后面", () => {
    const items = [...three.slice(0, 2), item({ metric: "requests", enabled: false, order: 2 })];
    const next = moveSegment(items, "today_usage:cost", 1);
    expect(next.map(segmentKey)).toEqual(["today_usage:tokens", "today_usage:cost", "today_usage:requests"]);
    expect(next.map((i) => i.order)).toEqual([0, 1, 2]);
    expect(moveSegment(items, "peak", 0)).toBe(items);
    // 三项首移末：顺移（arrayMove），非两两调换
    expect(moveSegment(three, "today_usage:cost", 2).map(segmentKey)).toEqual(["today_usage:tokens", "peak", "today_usage:cost"]);
  });
  it("loadSegmentAt：插到指定位置；已装载原样返回；已装 3 段仍可插入", () => {
    const two = three.slice(0, 2);
    expect(loadSegmentAt(two, byKey("peak"), 0).map(segmentKey)).toEqual(["peak", "today_usage:cost", "today_usage:tokens"]);
    expect(loadSegmentAt(two, byKey("today_usage:cost"), 1)).toBe(two);
    expect(loadSegmentAt(three, byKey("routed_platform"), 0).map(segmentKey)).toHaveLength(4);
  });
});

describe("拖拽装载 / 排序", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("候选芯片拖到已装载槽 → 插在该槽位置，持久化一次", async () => {
    await renderTab();
    drop("chip:peak", "slot:today_usage:tokens");
    expect(api.set).toHaveBeenCalledTimes(1);
    expect(sentKeys()).toEqual(["today_usage:cost", "peak", "today_usage:tokens"]);
  });

  it("候选芯片拖到空槽 → 追加末尾", async () => {
    await renderTab();
    drop("chip:peak", "empty:2");
    expect(sentKeys()).toEqual(["today_usage:cost", "today_usage:tokens", "peak"]);
  });

  it("已装 3 段仍可拖入第 4 段（无上限）；无落点不持久化", async () => {
    await renderTab({
      separator: "  ",
      items: [item({ metric: "cost", order: 0 }), item({ metric: "tokens", order: 1 }), item({ item_type: "peak", metric: null, order: 2 })],
    });
    drop("chip:today_usage:requests", null);
    expect(api.set).not.toHaveBeenCalled();
    drop("chip:routed_platform", "empty:3");
    expect(sentKeys()).toEqual(["today_usage:cost", "today_usage:tokens", "peak", "routed_platform"]);
  });

  it("槽内拖拽排序 → 新顺序持久化", async () => {
    await renderTab();
    drop("slot:today_usage:cost", "slot:today_usage:tokens");
    expect(api.set).toHaveBeenCalledTimes(1);
    expect(sentKeys()).toEqual(["today_usage:tokens", "today_usage:cost"]);
  });

  it("槽拖回候选库 = 移除（只翻 enabled）", async () => {
    await renderTab();
    drop("slot:today_usage:cost", "library");
    expect(sentKeys()).toEqual(["today_usage:tokens"]);
    expect(api.set.mock.calls[0][0].items).toHaveLength(2);
  });

  it("每个已装载槽有拖拽手柄（键盘排序入口），空槽无", async () => {
    const view = await renderTab();
    expect(view.getByTestId("tray-slot-handle-0").getAttribute("aria-roledescription")).toBe("sortable");
    expect(view.queryByTestId("tray-slot-handle-2")).toBeNull();
  });
});

// ── 票 02：分隔符（候选库 + 间隙投放）──
const sep = (display: string, patch: Partial<TrayItem> = {}) =>
  item({ item_type: "separator", metric: null, display, ...patch });
/** 按启用序列展开成 "key" / "|" 之类的可读序列（分隔符用 display）。 */
const seq = (items: TrayItem[]) =>
  items.filter((i) => i.enabled).sort((a, b) => a.order - b.order)
    .map((i) => (i.item_type === "separator" ? `[${i.display}]` : segmentKey(i)));
const abc = () => [
  item({ metric: "cost", order: 0 }), item({ metric: "tokens", order: 1 }),
  item({ item_type: "peak", metric: null, order: 2 }),
];

describe("分隔符候选与 key", () => {
  it("候选库含 4 个预设（| ｜ · -），key 互不相同且不撞已有 key", () => {
    const opts = segmentOptions([], t).filter((o) => o.item.item_type === "separator");
    expect(opts.map((o) => o.item.display)).toEqual(["|", "｜", "·", "-"]);
    const all = segmentOptions([], t).map((o) => o.key);
    expect(new Set(all).size).toBe(all.length);
  });
  it("segmentKey 对 separator 带 display 区分", () => {
    expect(segmentKey(sep("|"))).not.toBe(segmentKey(sep("｜")));
    expect(segmentKey(sep("·"))).toBe("separator:·");
  });
});

describe("setGapSeparator / gapSeparators", () => {
  it("投放到中间间隙：order 正确，不占段数", () => {
    const next = setGapSeparator(abc(), 1, "·");
    expect(seq(next)).toEqual(["today_usage:cost", "[·]", "today_usage:tokens", "peak"]);
    expect(next.map((i) => i.order)).toEqual([0, 1, 2, 3]);
    expect(gapSeparators(next)).toEqual([null, "·", null, null]);
  });
  it("首尾间隙也可放", () => {
    const next = setGapSeparator(setGapSeparator(abc(), 0, "-"), 3, "|");
    expect(seq(next)).toEqual(["[-]", "today_usage:cost", "today_usage:tokens", "peak", "[|]"]);
  });
  it("同一预设可重复投放到不同间隙；同间隙再投放 = 替换，至多一个", () => {
    let next = setGapSeparator(abc(), 1, "|");
    next = setGapSeparator(next, 2, "|");
    expect(next.filter((i) => i.item_type === "separator")).toHaveLength(2);
    next = setGapSeparator(next, 1, "｜");
    expect(gapSeparators(next)).toEqual([null, "｜", "|", null]);
    expect(next.filter((i) => i.item_type === "separator")).toHaveLength(2);
  });
  it("null = 拖走移除", () => {
    const next = setGapSeparator(setGapSeparator(abc(), 1, "·"), 1, null);
    expect(seq(next)).toEqual(["today_usage:cost", "today_usage:tokens", "peak"]);
    expect(next.some((i) => i.item_type === "separator")).toBe(false);
  });
  it("I15 关掉的旧分隔符项原样保留、不复活、不进 gaps", () => {
    const legacy = sep("—", { enabled: false, order: 3 });
    const next = setGapSeparator([...abc(), legacy], 1, "·");
    expect(gapSeparators(next)).toEqual([null, "·", null, null]);
    const kept = next.filter((i) => i.item_type === "separator" && !i.enabled);
    expect(kept).toHaveLength(1);
    expect(kept[0].display).toBe("—");
  });
});

describe("分隔符与段增删/移动共存", () => {
  it("moveSegment：分隔符留在原间隙位置", () => {
    const items = setGapSeparator(abc(), 1, "·");
    expect(seq(moveSegment(items, "today_usage:cost", 2))).toEqual(
      ["today_usage:tokens", "[·]", "peak", "today_usage:cost"],
    );
  });
  it("loadSegmentAt：新段插在该位置，原间隙分隔符仍在其右侧邻位（不丢）", () => {
    const items = setGapSeparator(abc(), 1, "·"); // cost [·] tokens peak
    const next = loadSegmentAt(items, byKey("today_usage:requests"), 1);
    expect(seq(next)).toEqual(["today_usage:cost", "today_usage:requests", "[·]", "today_usage:tokens", "peak"]);
  });
  it("toggleSegment 关掉一段：其左侧间隙一并移除，其余分隔符保留，尾部分隔符仍在尾部", () => {
    let items = setGapSeparator(abc(), 1, "·");
    items = setGapSeparator(items, 3, "|");
    const next = toggleSegment(items, byKey("today_usage:tokens"));
    expect(seq(next)).toEqual(["today_usage:cost", "peak", "[|]"]);
  });
  it("toggleSegment 追加新段：尾部分隔符仍在尾部", () => {
    const items = setGapSeparator(abc(), 3, "|");
    expect(seq(toggleSegment(items, byKey("today_usage:requests")))).toEqual(
      ["today_usage:cost", "today_usage:tokens", "peak", "today_usage:requests", "[|]"],
    );
  });
});

describe("分隔符拖拽", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });
  const sentSeq = () => seq(api.set.mock.calls[0][0].items as TrayItem[]);

  it("分隔符芯片拖到间隙 → 持久化 separator 项在正确 order", async () => {
    await renderTab();
    drop("chip:separator:·", "gap:1");
    expect(api.set).toHaveBeenCalledTimes(1);
    expect(sentSeq()).toEqual(["today_usage:cost", "[·]", "today_usage:tokens"]);
  });
  it("同一预设可重复拖入（库内芯片不会因已装载而禁用）", async () => {
    const view = await renderTab({
      separator: "  ",
      items: [item({ metric: "cost", order: 0 }), sep("·", { order: 1 }), item({ metric: "tokens", order: 2 })],
    });
    expect(view.getByTestId("tray-chip-separator:·").getAttribute("data-state")).toBe("");
    drop("chip:separator:·", "gap:2");
    expect(sentSeq()).toEqual(["today_usage:cost", "[·]", "today_usage:tokens", "[·]"]);
  });
  it("首尾间隙可投放", async () => {
    await renderTab();
    drop("chip:separator:|", "gap:0");
    expect(sentSeq()).toEqual(["[|]", "today_usage:cost", "today_usage:tokens"]);
  });
  it("间隙内分隔符拖回候选库 = 移除；拖到另一间隙 = 移动", async () => {
    const cfg = {
      separator: "  ",
      items: [item({ metric: "cost", order: 0 }), sep("·", { order: 1 }), item({ metric: "tokens", order: 2 })],
    };
    await renderTab(cfg);
    drop("gapsep:1", "library");
    expect(sentSeq()).toEqual(["today_usage:cost", "today_usage:tokens"]);
    vi.clearAllMocks();
    await renderTab(cfg);
    drop("gapsep:1", "gap:2");
    expect(sentSeq()).toEqual(["today_usage:cost", "today_usage:tokens", "[·]"]);
  });
  it("分隔符芯片不能落在槽上（无效落点不持久化）；预览条显示分隔符", async () => {
    const view = await renderTab({
      separator: "  ",
      items: [item({ metric: "cost", order: 0 }), sep("｜", { order: 1 }), item({ metric: "tokens", order: 2 })],
    });
    drop("chip:separator:|", "slot:today_usage:cost");
    expect(api.set).not.toHaveBeenCalled();
    expect(view.getByTestId("tray-preview-sep-1").textContent).toBe("｜");
  });
  it("旧版已禁用的分隔符仍显示「已保留未启用」提示，不进装载序列", async () => {
    const view = await renderTab({
      separator: "  ",
      items: [item({ metric: "cost", order: 0 }), sep("|", { order: 1, enabled: false })],
    });
    expect(view.getByTestId("tray-kept-disabled")).toBeTruthy();
    expect(view.queryByTestId("tray-preview-sep-1")).toBeNull();
  });
});

// ── 颜色「自动」模式（票 托盘-颜色分隔符槽位/03）──

describe("颜色 auto 模式", () => {
  it("新建金额段（今日花费）显式 unit_position=prefix，非金额段不写", () => {
    const opts = segmentOptions([plat as never], t);
    expect(opts.find((o) => o.key === "today_usage:cost")!.item.unit_position).toBe("prefix");
    expect(opts.find((o) => o.key === "today_usage:tokens")!.item.unit_position).toBeNull();
  });

  it("新装载候选段默认 auto；已有段（item() 夹具 follow）不被改写", () => {
    for (const o of segmentOptions([plat as never], t)) expect(o.item.color).toEqual({ mode: "auto", value: "" });
    const next = toggleSegment([item({ metric: "cost", enabled: false })], byKey("today_usage:cost"));
    expect(next[0].color).toEqual({ mode: "follow", value: "" });
  });

  it("segColor：auto 按 level 取界面 token，无 level -> 菜单栏前景；follow 不受 level 影响", () => {
    const auto = { mode: "auto", value: "" };
    expect(segColor(auto, "danger")).toBe("var(--mb-red)");
    expect(segColor(auto, "warning")).toBe("var(--mb-orange)");
    expect(segColor(auto, "success")).toBe("var(--mb-green)");
    expect(segColor(auto, "neutral")).toBe("var(--mb-fg-strong)");
    expect(segColor(auto)).toBe("var(--mb-fg-strong)");
    expect(segColor({ mode: "follow", value: "" }, "danger")).toBe("var(--mb-fg-strong)");
  });

  it("previewLevel：余额取后端 balance_level，coding 按 usageColor 进度差，today 无 level", () => {
    const now = 1_800_000_000_000;
    const coding = (util: number) => ({
      id: 7, balance_level: "neutral",
      est_coding_plan: JSON.stringify({ tiers: [{ name: "five_hour", est_utilization: util, window_start: now - 3_600_000 }] }),
    }) as unknown as Platform;
    const bal = (lv: string) => ({ id: 7, balance_level: lv, est_coding_plan: "" }) as unknown as Platform;
    const pi = platformItem({ display: "balance" });
    expect(previewLevel(pi, [bal("green")], now)).toBe("success");
    expect(previewLevel(pi, [bal("yellow")], now)).toBe("warning");
    expect(previewLevel(pi, [bal("red")], now)).toBe("danger");
    expect(previewLevel(pi, [bal("")], now)).toBe("neutral");
    const ci = platformItem({ metrics: ["coding"] });
    expect(previewLevel(ci, [coding(10)], now)).toBe("success"); // 时间过 20%，差 -10
    expect(previewLevel(ci, [coding(20)], now)).toBe("warning");
    expect(previewLevel(ci, [coding(40)], now)).toBe("danger");
    expect(previewLevel(item({ metric: "cost" }), [], now)).toBe("neutral");
  });
});

// 跨层 golden：与 Rust platform_item_level_golden_cases_match 共读 tray_level_golden.json。
describe("previewLevel 对拍 Rust platform_item_level", () => {
  const now = 1_800_000_000_000;
  type LevelCase = {
    name: string; display?: string; metrics?: string[]; balance_level?: string; manual?: boolean;
    tiers?: { name: string; util: number; elapsed_ms?: number }[]; expected: string;
  };
  it.each(levelGolden as LevelCase[])("$name", (c) => {
    const p = {
      id: 7,
      balance_level: c.balance_level ?? "",
      est_coding_plan: c.tiers
        ? JSON.stringify({ tiers: c.tiers.map((t) => ({ name: t.name, est_utilization: t.util, window_start: t.elapsed_ms ? now - t.elapsed_ms : 0 })) })
        : "",
    } as unknown as Platform;
    const it = platformItem({ ...(c.display ? { display: c.display } : {}), ...(c.metrics ? { metrics: c.metrics } : {}) });
    expect(previewLevel(it, [p], now)).toBe(c.expected);
  });
});

// ── 托盘倒计时三格式（与 Rust format_countdown 同语义；nowMs 注入）──

describe("formatCountdown", () => {
  const now = 1_800_000_000_000;
  const m = 60_000;
  // [剩余毫秒, compact, cn, clock]
  const cases: Array<[number, string, string, string]> = [
    [59_000, "1m", "1分", "0:01"],
    [60 * m, "1h", "1小时", "1:00"],
    [135 * m, "2h15m", "2小时15分", "2:15"],
    [23 * 60 * m + 59 * m, "23h59m", "23小时59分", "23:59"],
    [24 * 60 * m, "1d", "1天", "1天 0:00"],
    [24 * 60 * m + 20 * m, "1d20m", "1天20分", "1天 0:20"],
    [27 * 60 * m + 20 * m, "1d3h20m", "1天3小时20分", "1天 3:20"],
    [24 * 60 * m + 1, "1d1m", "1天1分", "1天 0:01"],
  ];
  it.each(cases)("剩余 %i ms 三格式", (ms, compact, cn, clock) => {
    expect(formatCountdown(now + ms, now, "compact")).toBe(compact);
    expect(formatCountdown(now + ms, now, "cn")).toBe(cn);
    expect(formatCountdown(now + ms, now, "clock")).toBe(clock);
    expect(formatCountdown(now + ms, now, undefined)).toBe(cn);
  });

  it("过期 / 恰好到点 → null", () => {
    expect(formatCountdown(now, now, "cn")).toBe(null);
    expect(formatCountdown(now - 1, now, "clock")).toBe(null);
  });
});

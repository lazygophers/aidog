// 票 I15：托盘配置从二维网格降成「最多挑 3 项」。这里锁的是迁移规则本身 ——
// 取消勾选只翻 enabled，绝不删项；满 3 项后再勾无效。
// 票 托盘设置-重设计/01：页面骨架 + 实时预览条（C 方向），组件级 vitest 锚 testid/data-state。
import { beforeEach, describe, it, expect, vi } from "vitest";
import { render, waitFor, fireEvent, act } from "../test/render";
import {
  MAX_SEGMENTS,
  segmentKey,
  segmentOptions,
  toggleSegment,
  moveSegment,
  loadSegmentAt,
  previewValue,
  trayMoney,
  type SegmentOption,
} from "./TrayConfigTab";
import type { TrayItem, TodayStats, Platform } from "../services/api";
import type { DisplayCurrency } from "../services/api/types";
import { setDisplayCurrency, USD_DISPLAY } from "../utils/displayCurrency";
import { setBalanceCurrencies } from "../utils/balanceCurrency";

const mk = (code: string, symbol: string, decimals: number, rate: number): DisplayCurrency =>
  ({ code, symbol, decimals, rate, table: [] });

const t = ((key: string, fallback?: string) => fallback ?? key) as never;

function item(patch: Partial<TrayItem>): TrayItem {
  return {
    item_type: "today_usage", platform_id: null, display: "", metric: "cost", label: null,
    decimals: null, color: { mode: "follow", value: "" }, font_size: 9, line_mode: "two",
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

  it("已满 3 项时再勾无效（原样返回）", () => {
    const items = [
      item({ metric: "cost", order: 0 }),
      item({ metric: "tokens", order: 1 }),
      item({ item_type: "peak", metric: null, order: 2 }),
    ];
    expect(items.filter((i) => i.enabled)).toHaveLength(MAX_SEGMENTS);
    expect(toggleSegment(items, byKey("routed_platform"))).toBe(items);
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

// 跨层对拍：期望串逐字抄自 Rust test_tray_render.rs 的 tray_money_* 用例（同输入同币种参数）。
describe("trayMoney（对拍 Rust tray_money）", () => {
  const usd = USD_DISPLAY;
  it.each([
    // tray_money_usd_matches_legacy_text
    [10.10, usd, 2, "$10.1"],
    [0.0, usd, 2, "$0"],
    [0.123456789, usd, 5, "$0.12346"],
    // tray_money_cny_jpy_chf
    [10.0, mk("CNY", "¥", 2, 7.0), 2, "¥70"],
    [0.0123456, mk("CNY", "¥", 2, 7.0), 5, "¥0.08642"],
    [1234.567, mk("JPY", "JP¥", 0, 150.0), 5, "JP¥185185"],
    [0.004, mk("JPY", "JP¥", 0, 150.0), 2, "JP¥1"],
    [10.0, mk("CHF", "CHF ", 2, 0.9), 2, "CHF 9"],
  ])("%s -> %s", (n, cur, d, expected) => {
    expect(trayMoney(n as number, cur as DisplayCurrency, d as number)).toBe(expected);
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
  peak: { platform_name: string; in_peak: boolean; multiplier: number } | null = null,
  trend: { buckets: unknown[]; series: unknown[] } = { buckets: [], series: [] },
) {
  const { TrayConfigTab } = await import("./TrayConfigTab");
  api.list.mockResolvedValue([]);
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

  it("页头 n/3 计数反映已装载段数", async () => {
    const view = await renderTab();
    expect(view.getByTestId("tray-count").textContent).toContain("2");
    expect(view.getByTestId("tray-count").textContent).toContain("3");
  });

  it("装载槽 3 个：已装载槽显示段值，空槽标 empty", async () => {
    const view = await renderTab();
    expect(view.getByTestId("tray-slot-0").getAttribute("data-state")).toBe("filled");
    expect(view.getByTestId("tray-slot-0").textContent).toContain("$1.25");
    expect(view.getByTestId("tray-slot-1").getAttribute("data-state")).toBe("filled");
    expect(view.getByTestId("tray-slot-2").getAttribute("data-state")).toBe("empty");
  });

  it("候选库芯片带 loaded/空 data-state（装载交互见票 02 组）", async () => {
    const view = await renderTab();
    expect(view.getByTestId("tray-chip-today_usage:cost").getAttribute("data-state")).toBe("loaded");
    expect(view.getByTestId("tray-chip-today_usage:tokens").getAttribute("data-state")).toBe("loaded");
    expect(view.getByTestId("tray-chip-peak").getAttribute("data-state")).toBe("");
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
    const view = await renderTab(peakCfg, { platform_name: "Acme", in_peak: true, multiplier: 2 });
    await waitFor(() => expect(view.getByTestId("tray-peak-badge").textContent).toContain("Acme ×2"));
    expect(view.getByTestId("tray-segment-peak").textContent).toContain("峰");
  });

  it("非高峰 → 徽标隐藏，预览段为「平」", async () => {
    const view = await renderTab(peakCfg, { platform_name: "Acme", in_peak: false, multiplier: 1 });
    await waitFor(() => expect(api.peakStatus).toHaveBeenCalled());
    expect(view.queryByTestId("tray-peak-badge")).toBeNull();
    expect(view.getByTestId("tray-segment-peak").textContent).toContain("平");
  });

  it("无最近命中平台 → 徽标隐藏", async () => {
    const view = await renderTab(peakCfg, null);
    await waitFor(() => expect(api.peakStatus).toHaveBeenCalled());
    expect(view.queryByTestId("tray-peak-badge")).toBeNull();
  });

  it("趋势卡按 statsApi.query 的全局 7 日按天查询渲染", async () => {
    const view = await renderTab(undefined, null, {
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

  it("满 3 段：未选项置灰 locked 且点击无效", async () => {
    const view = await renderTab({
      separator: "  ",
      items: [
        item({ metric: "cost", order: 0 }),
        item({ metric: "tokens", order: 1 }),
        item({ item_type: "peak", metric: null, order: 2 }),
      ],
    });
    const chip = view.getByTestId("tray-chip-routed_platform");
    expect(chip.getAttribute("data-state")).toBe("locked");
    fireEvent.click(chip);
    expect(api.set).not.toHaveBeenCalled();
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
    expect(seg).toContain("var(--destructive)");
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
  it("loadSegmentAt：插到指定位置；已装载 / 满槽原样返回", () => {
    const two = three.slice(0, 2);
    expect(loadSegmentAt(two, byKey("peak"), 0).map(segmentKey)).toEqual(["peak", "today_usage:cost", "today_usage:tokens"]);
    expect(loadSegmentAt(two, byKey("today_usage:cost"), 1)).toBe(two);
    expect(loadSegmentAt(three, byKey("routed_platform"), 0)).toBe(three);
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

  it("满槽时拖入无效（沿用点选上限），无落点也不持久化", async () => {
    await renderTab({
      separator: "  ",
      items: [item({ metric: "cost", order: 0 }), item({ metric: "tokens", order: 1 }), item({ item_type: "peak", metric: null, order: 2 })],
    });
    drop("chip:routed_platform", "slot:peak");
    drop("chip:today_usage:requests", null);
    expect(api.set).not.toHaveBeenCalled();
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

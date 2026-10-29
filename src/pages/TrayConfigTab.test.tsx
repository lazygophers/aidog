// 票 I15：托盘配置从二维网格降成「最多挑 3 项」。这里锁的是迁移规则本身 ——
// 取消勾选只翻 enabled，绝不删项；满 3 项后再勾无效。
// 票 托盘设置-重设计/01：页面骨架 + 实时预览条（C 方向），组件级 vitest 锚 testid/data-state。
import { beforeEach, describe, it, expect, vi } from "vitest";
import { render, waitFor, fireEvent } from "../test/render";
import {
  MAX_SEGMENTS,
  segmentKey,
  segmentOptions,
  toggleSegment,
  previewValue,
  type SegmentOption,
} from "./TrayConfigTab";
import type { TrayItem, TodayStats } from "../services/api";

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

  it("命中平台 / 高峰由后端实时算，前端占位", () => {
    expect(previewValue(item({ item_type: "routed_platform", metric: null }), [], null)).toBe("—");
    expect(previewValue(item({ item_type: "peak", metric: null }), [], null)).toBe("—");
  });
});

// ── 票 01：页面骨架 + 实时预览条 ──

const api = vi.hoisted(() => ({
  list: vi.fn(),
  get: vi.fn(),
  todayStats: vi.fn(),
  set: vi.fn(),
  onProxyLogUpdated: vi.fn(),
}));
vi.mock("../services/api", () => ({
  platformApi: { list: api.list },
  trayConfigApi: { get: api.get, set: api.set, todayStats: api.todayStats },
  onProxyLogUpdated: api.onProxyLogUpdated,
}));

const stats = { tokens: 1240000, input_tokens: 0, output_tokens: 0, cache_tokens: 0, cache_rate: 74, cost: 1.25, total_requests: 86 };

async function renderTab(cfg?: { separator: string; items: TrayItem[] }) {
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
  api.onProxyLogUpdated.mockReturnValue(() => {});
  const view = render(<TrayConfigTab />);
  await waitFor(() => expect(view.getByTestId("tray-menubar")).toBeTruthy());
  return view;
}

describe("TrayConfigTab 骨架与预览（票 01）", () => {
  beforeEach(() => {
    vi.clearAllMocks();
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
});

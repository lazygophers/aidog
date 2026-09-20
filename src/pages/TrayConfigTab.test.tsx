// 票 I15：托盘配置从二维网格降成「最多挑 3 项」。这里锁的是迁移规则本身 ——
// 取消勾选只翻 enabled，绝不删项；满 3 项后再勾无效。
import { describe, it, expect } from "vitest";
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

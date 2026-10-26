// GroupConsoleList 就近测试：pgtop 计数 + 组卡渲染 + 点卡开详情 + 新建卡。
// 选择器锚稳定 testid；文案断言用 i18n key（test/render 回退 key）。
import { describe, it, expect, vi } from "vitest";
import { fireEvent } from "@testing-library/react";
import { render, screen } from "../../test/render";

vi.mock("../../../services/api", async () => {
  const actual = await vi.importActual<typeof import("../../../services/api")>("../../../services/api");
  return {
    ...actual,
    platformApi: Object.assign(Object.create(Object.getPrototypeOf(actual.platformApi)), actual.platformApi, {
      purgeDisabledPreview: vi.fn().mockResolvedValue([]),
    }),
  };
});

import { GroupConsoleList } from "./GroupConsoleList";
import type { GroupDetail, PlatformUsageStats } from "../../../services/api";

const detail = {
  group: { id: 1, name: "main", group_key: "gk", routing_mode: "failover", is_default: true },
  platforms: [{ platform: { id: 1 } }, { platform: { id: 2 } }],
  model_mappings: [],
} as never;
const groupStats = {
  gk: { total_requests: 1000, success_count: 990, total_input_tokens: 1, total_output_tokens: 1, total_cache_tokens: 0, cache_rate: 0, recent_failures: 0, recent_total: 0, total_cost: 4.8, today_tokens: 1248, today_cost: 4.8 },
} as never as Record<string, PlatformUsageStats>;

const base = {
  details: [detail] as never,
  groupStats,
  loading: false,
  t: ((k: string, fallback?: string) => fallback ?? k) as never,
  proxyBaseUrl: "http://127.0.0.1:9890",
  onOpenDetail: vi.fn(),
  onCreateGroup: vi.fn(),
  onPurgeAllGroups: vi.fn().mockResolvedValue(undefined),
};

describe("GroupConsoleList", () => {
  it("pgtop 计数 + 组卡渲染（默认星标卡 hp 条）", () => {
    render(<GroupConsoleList {...base} />);
    expect(screen.getByTestId("groups-console-top")).toBeTruthy();
    expect(screen.getByTestId("group-card")).toBeTruthy();
    expect(screen.getByTestId("group-card-hp")).toBeTruthy();
  });

  it("点组卡 → onOpenDetail(1)；点新建卡 → onCreateGroup", () => {
    render(<GroupConsoleList {...base} />);
    fireEvent.click(screen.getByTestId("group-card"));
    expect(base.onOpenDetail).toHaveBeenCalledWith(1);
    fireEvent.click(screen.getByTestId("group-card-new"));
    expect(base.onCreateGroup).toHaveBeenCalled();
  });

  it("loading 态不渲染组卡", () => {
    render(<GroupConsoleList {...base} loading details={[]} as never />);
    expect(screen.queryByTestId("group-card")).toBeNull();
  });
});

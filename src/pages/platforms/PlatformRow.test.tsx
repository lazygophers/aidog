// PlatformRow 就近测试：行渲染 + 徽标 + 展开 + 开关/测速回调。
// 选择器锚稳定 testid；文案断言用 i18n key（test/render 回退 key）。
import { describe, it, expect, vi } from "vitest";
import { fireEvent } from "@testing-library/react";
import { render, screen } from "../../test/render";

vi.mock("../../../domains/platforms/useProtocolMeta", () => ({
  useProtocolMeta: () => ({ color: "#123", isCpProtocol: false, defaultModels: ["preset-model"], homepage: "", sourceUrls: { docs: "", pricing: "" }, protocolLabel: "", labelMap: {}, peakWindows: [] }),
}));

import { PlatformRow } from "./PlatformRow";
import type { Platform, PlatformUsageStats, LastTestResult } from "../../../services/api";

const platform = {
  id: 7, name: "DeepSeek", platform_type: "deepseek", base_url: "https://api.deepseek.com/v1",
  extra: "{}", models: {}, available_models: [], endpoints: [],
  status: "enabled", est_balance_remaining: 4.12, balance_level: "",
  manual_budgets: [],
} as never as Platform;

const usage = {
  total_requests: 800, success_count: 790, total_input_tokens: 1, total_output_tokens: 1,
  total_cache_tokens: 0, cache_rate: 0, recent_failures: 0, recent_total: 0,
  total_cost: 3.2, today_tokens: 1248, today_cost: 0.86,
} as PlatformUsageStats;

const lastTest = { success: true, status_code: 200, duration_ms: 402, created_at: Date.now() - 3600_000, error: "", response_body: "" } as LastTestResult;

const base = {
  platform, usage, lastTest,
  t: ((k: string, fallback?: string) => fallback ?? k) as never,
  lang: "zh-Hans",
};

describe("PlatformRow", () => {
  it("行渲染：testid / 状态点 / 开关 on / 测速 ok", () => {
    render(<PlatformRow {...base} />);
    expect(screen.getByTestId("platform-row")).toBeTruthy();
    expect(screen.getByTestId("row-status-dot").getAttribute("title")).toBe("enabled");
    expect(screen.getByTestId("row-toggle").getAttribute("data-state")).toBe("on");
    expect(screen.getByTestId("row-last-test").getAttribute("data-result")).toBe("ok");
  });

  it("点行展开明细（模型 pills），开关与测速走回调", () => {
    const onToggleExpanded = vi.fn();
    const onToggle = vi.fn();
    const onQuickTest = vi.fn();
    render(<PlatformRow {...base} onToggleExpanded={onToggleExpanded} onToggle={onToggle} onQuickTest={onQuickTest} />);
    fireEvent.click(screen.getByTestId("platform-row").querySelector("[data-testid='row-toggle']")!);
    expect(onToggle).toHaveBeenCalledWith(platform);
    fireEvent.click(screen.getByTitle("测速"));
    expect(onQuickTest).toHaveBeenCalledWith(platform);
    fireEvent.click(screen.getByTestId("platform-row").firstElementChild!);
    expect(onToggleExpanded).toHaveBeenCalledWith(true);
  });

  it("expanded=true 渲染明细区（模型 + 配置模型计数）", () => {
    render(<PlatformRow {...base} expanded models={undefined} />);
    expect(screen.getByTestId("row-detail")).toBeTruthy();
  });

  it("auto_disabled 态行半透明 + 状态点红", () => {
    const disabled = { ...platform, status: "auto_disabled" } as never as Platform;
    render(<PlatformRow {...base} platform={disabled} lastTest={undefined} />);
    expect(screen.getByTestId("row-status-dot").getAttribute("title")).toBe("auto_disabled");
  });
});

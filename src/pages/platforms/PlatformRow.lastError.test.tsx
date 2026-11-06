// PlatformRow「最近一次请求结果」就近测试：渲染真组件，testid + data-result 断言，不断言 locale 文案。
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "../../test/render";

vi.mock("../../../domains/platforms/useProtocolMeta", () => ({
  useProtocolMeta: () => ({ color: "#123", isCpProtocol: false, defaultModels: [], homepage: "", sourceUrls: { docs: "", pricing: "" }, protocolLabel: "", labelMap: {}, peakWindows: [] }),
}));
vi.mock("../../services/api/currency", () => ({
  DISPLAY_CURRENCY_EVENT: "display-currency-updated",
  displayCurrencyApi: { get: vi.fn().mockResolvedValue({ code: "USD", symbol: "$", decimals: 2, rate: 1, table: [] }) },
}));

import { PlatformRow } from "./PlatformRow";
import type { Platform } from "../../../services/api";

const mk = (over: Record<string, unknown>) => ({
  id: 7, name: "DeepSeek", platform_type: "deepseek", base_url: "https://api.deepseek.com/v1",
  extra: "{}", models: {}, available_models: [], endpoints: [], status: "enabled",
  est_balance_remaining: 1, balance_level: "", manual_budgets: [], last_error: "", last_error_at: 0, ...over,
} as never as Platform);
const base = { t: ((k: string, f?: string) => f ?? k) as never, lang: "zh-Hans", showLastOutcome: true };

describe("PlatformRow 最近一次请求结果", () => {
  it("last_error 非空：失败 + 原因 + 完整原因在 title", () => {
    render(<PlatformRow {...base} platform={mk({ last_error: "HTTP 429 rate limited", last_error_at: Date.now() - 120_000 })} />);
    const el = screen.getByTestId("row-last-outcome");
    expect(el.getAttribute("data-result")).toBe("fail");
    expect(screen.getByTestId("row-last-outcome-reason").textContent).toContain("HTTP 429 rate limited");
    expect(el.getAttribute("title")).toContain("HTTP 429 rate limited");
    expect(screen.getByTestId("row-last-outcome-time").textContent).toBeTruthy();
  });

  it("last_error 为空：正常，无原因、无时间", () => {
    render(<PlatformRow {...base} platform={mk({})} />);
    expect(screen.getByTestId("row-last-outcome").getAttribute("data-result")).toBe("ok");
    expect(screen.queryByTestId("row-last-outcome-reason")).toBeNull();
    expect(screen.queryByTestId("row-last-outcome-time")).toBeNull();
  });

  it("未传 showLastOutcome（平台列表页）：不渲染", () => {
    render(<PlatformRow {...base} showLastOutcome={undefined} platform={mk({ last_error: "x", last_error_at: 1 })} />);
    expect(screen.queryByTestId("row-last-outcome")).toBeNull();
  });
});

// quota-ia 票 03：合区 Tab 互斥切换（表单内只记忆目标，数据不动；保存清库在 Rust 侧测）。
// + 配额查询「测试」按钮：无脚本配置禁用、点击调 quotaApi.query、success/failure 展示。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/react";
import { render } from "../../test/render";
import { QuotaSection } from "./formSections";
import type { QuotaScriptVariant } from "../../domains/platforms";
import type { PlatformQuota } from "../../services/api";

const { quotaQueryMock } = vi.hoisted(() => ({ quotaQueryMock: vi.fn() }));
vi.mock("../../services/api", async (importOriginal) => {
  const orig = await importOriginal<typeof import("../../services/api")>();
  return { ...orig, quotaApi: { ...orig.quotaApi, query: quotaQueryMock } };
});

const t = ((key: string, fallback?: string) => fallback ?? key) as unknown as Parameters<typeof QuotaSection>[0]["t"];

const VARIANT: QuotaScriptVariant = {
  id: "default",
  name: { "en-US": "Official" },
  requires: [],
  returns: { balance: true },
  script: "",
};

function okQuota(): PlatformQuota {
  return {
    success: true,
    error: "legacy fields ignored",
    queried_at: 1_700_000_000_000,
    balance: { remaining: 12.5, total: 100, used: 87.5, currency: "CNY", is_valid: true },
    coding_plan: { tiers: [{ name: "five_hour", utilization: 42, resets_at: null, limit: null, remaining: null }], level: null },
  };
}

function renderSection(opts: {
  quotaSource?: "auto" | "manual";
  budgets?: { amount: number }[];
  customScript?: string;
  baseUrl?: string;
  apiKey?: string;
} = {}) {
  const onSourceChange = vi.fn();
  const { container, getByText } = render(
    <QuotaSection
      quotaSource={opts.quotaSource ?? "auto"}
      onSourceChange={onSourceChange}
      protocol="deepseek"
      variants={[VARIANT]}
      variantId=""
      onVariantChange={vi.fn()}
      customScript={opts.customScript ?? ""}
      onCustomScriptChange={vi.fn()}
      requires={{}}
      onRequiresChange={vi.fn()}
      baseUrl={opts.baseUrl ?? "https://x.test"}
      apiKey={opts.apiKey ?? "k-123"}
      budgets={(opts.budgets ?? []) as never}
      setBudgets={vi.fn()}
      editing
      t={t}
    />
  );
  return { container, getByText, onSourceChange };
}

/** Radix TabsTrigger 在 mouseDown 阶段切值（click 不触发，先例 ModelInfoTab.test.tsx:154）。 */
function switchTab(r: ReturnType<typeof renderSection>, name: string) {
  fireEvent.mouseDown(r.getByText(name), { button: 0 });
}

beforeEach(() => {
  quotaQueryMock.mockReset();
});

describe("QuotaSection（quota-ia 票 03）", () => {
  it("auto tab 渲染脚本变体下拉；manual tab 渲染预算行", () => {
    const auto = renderSection();
    expect(auto.container.textContent).toContain("脚本变体");

    const manual = renderSection({ quotaSource: "manual", budgets: [{ amount: 5 }] });
    expect(manual.container.textContent).toContain("添加限额");
  });

  it("对侧有数据：切 tab 弹确认，确认后才记忆目标", () => {
    // auto + 自定义脚本 → 切 manual 需确认（弹窗走 portal 落 document.body，用 screen 查）
    const r = renderSection({ customScript: "function q(){}" });
    switchTab(r, "手动预算");
    expect(r.onSourceChange).not.toHaveBeenCalled();
    expect(screen.getByText("切换到手动预算？")).not.toBeNull();
    fireEvent.click(screen.getByText("切换"));
    expect(r.onSourceChange).toHaveBeenCalledWith("manual");
  });

  it("对侧无数据：静默切换，不弹确认", () => {
    // auto 无任何脚本配置 → 切 manual 直接切
    const r = renderSection();
    switchTab(r, "手动预算");
    expect(r.onSourceChange).toHaveBeenCalledWith("manual");
    expect(screen.queryByText("切换到手动预算？")).toBeNull();
  });

  it("manual 有预算 → 切 auto 也弹确认", () => {
    const r = renderSection({ quotaSource: "manual", budgets: [{ amount: 5 }] });
    switchTab(r, "自动脚本");
    expect(r.onSourceChange).not.toHaveBeenCalled();
    expect(screen.getByText("切换到自动脚本？")).not.toBeNull();
  });
});

describe("QuotaSection 测试按钮", () => {
  it("无脚本配置：按钮禁用并显示提示", () => {
    const r = renderSection();
    expect(r.container.querySelector('[data-testid="quota-test-button"]')).not.toBeNull();
    expect(r.container.querySelector('[data-testid="quota-test-button"]')).toBeDisabled();
    expect(r.container.querySelector('[data-testid="quota-test-hint"]')?.textContent).toContain("查询脚本");
  });

  it("点击后调 quotaApi.query 一次；查询中防重复点击", async () => {
    let release!: (v: PlatformQuota) => void;
    quotaQueryMock.mockReturnValue(new Promise<PlatformQuota>((res) => { release = res; }));
    const r = renderSection({ customScript: "function q(){}" });
    const btn = r.container.querySelector('[data-testid="quota-test-button"]') as HTMLButtonElement;
    expect(btn).not.toBeNull();
    expect(btn).toBeEnabled();
    fireEvent.click(btn);
    fireEvent.click(btn);
    expect(quotaQueryMock).toHaveBeenCalledTimes(1);
    expect(quotaQueryMock).toHaveBeenCalledWith("https://x.test", "k-123", undefined);
    release({ success: false, error: "x", queried_at: 0, balance: null, coding_plan: null });
    await waitFor(() => expect(r.container.querySelector('[data-testid="quota-test-result"]')).not.toBeNull());
  });

  it("success：展示余额、档位利用率、查询时间；error 作附注小字", async () => {
    quotaQueryMock.mockResolvedValue(okQuota());
    const r = renderSection({ customScript: "function q(){}" });
    fireEvent.click(r.container.querySelector('[data-testid="quota-test-button"]') as HTMLButtonElement);
    const result = await waitFor(() => {
      const el = r.container.querySelector('[data-testid="quota-test-result"]');
      expect(el).not.toBeNull();
      return el as HTMLElement;
    });
    const txt = result.textContent ?? "";
    expect(txt).toContain("12.5");
    expect(txt).toContain("CNY");
    expect(txt).toContain("five_hour");
    expect(txt).toContain("42");
    expect(txt).toContain("查询时间");
    expect(screen.getByTestId("quota-test-note")).not.toBeNull();
    expect(screen.getByTestId("quota-test-note").textContent).toContain("legacy fields ignored");
  });

  it("failure：error 原文按错误样式展示", async () => {
    quotaQueryMock.mockResolvedValue({ success: false, error: "boom", queried_at: 0, balance: null, coding_plan: null } as PlatformQuota);
    const r = renderSection({ customScript: "currently configured" });
    fireEvent.click(r.container.querySelector('[data-testid="quota-test-button"]') as HTMLButtonElement);
    const result = await waitFor(() => {
      const el = r.container.querySelector('[data-testid="quota-test-result"]');
      expect(el).not.toBeNull();
      return el as HTMLElement;
    });
    expect(result.querySelector('[data-testid="quota-test-error"]')?.textContent).toContain("boom");
  });
});

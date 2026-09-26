// quota-ia 票 03：合区 Tab 互斥切换（表单内只记忆目标，数据不动；保存清库在 Rust 侧测）。
import { describe, it, expect, vi } from "vitest";
import { fireEvent, screen } from "@testing-library/react";
import { render } from "../../test/render";
import { QuotaSection } from "./formSections";
import type { QuotaScriptVariant } from "../../domains/platforms";

const t = ((key: string, fallback?: string) => fallback ?? key) as unknown as Parameters<typeof QuotaSection>[0]["t"];

const VARIANT: QuotaScriptVariant = {
  id: "default",
  name: { "en-US": "Official" },
  requires: [],
  returns: { balance: true },
  script: "",
};

function renderSection(opts: { quotaSource?: "auto" | "manual"; budgets?: { amount: number }[]; customScript?: string } = {}) {
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

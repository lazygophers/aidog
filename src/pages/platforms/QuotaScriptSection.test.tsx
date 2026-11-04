// 行为断言（区块渲染与否 / 自定义脚本编辑器出现与否），不按文案文字断言。
import { describe, it, expect, vi } from "vitest";
import { render } from "../../test/render";
import { QuotaScriptFields, QUOTA_CUSTOM_VARIANT } from "./formSections";
import type { QuotaScriptVariant } from "../../domains/platforms";
import { QUOTA_SCRIPT_SAMPLE, isUserQuotaScript } from "../../services/api";

const t = ((key: string, fallback?: string) => fallback ?? key) as unknown as Parameters<typeof QuotaScriptFields>[0]["t"];

function renderSection(variants: QuotaScriptVariant[], customScript = "", variantId = "", protocol: "bailian_coding" | "newapi" = "bailian_coding", requires: Record<string, string> = {}) {
  const onCustomScriptChange = vi.fn();
  const customOnly = variants.length === 0;
  const hasCustom = isUserQuotaScript(customScript);
  const idValid = variants.some(v => v.id === variantId);
  const selection = hasCustom || customOnly || variantId === QUOTA_CUSTOM_VARIANT
    ? QUOTA_CUSTOM_VARIANT
    : idValid ? variantId : (variants[0]?.id ?? "");
  const selVariant = selection === QUOTA_CUSTOM_VARIANT ? null : variants.find(v => v.id === selection) ?? null;
  const { container } = render(
    <QuotaScriptFields
      protocol={protocol}
      variants={variants}
      onVariantChange={vi.fn()}
      customScript={customScript}
      onCustomScriptChange={onCustomScriptChange}
      requires={requires}
      onRequiresChange={vi.fn()}
      t={t}
      customOnly={customOnly}
      selection={selection}
      fellBack={false}
      selVariant={selVariant}
    />
  );
  return { container, onCustomScriptChange };
}

const VARIANT: QuotaScriptVariant = {
  id: "default",
  name: { "en-US": "Official" },
  requires: [],
  returns: { balance: true },
  script: "",
};

describe("QuotaScriptFields", () => {
  it("registry 无内置变体：仍渲染区块并直接给出自定义脚本编辑器（否则没法给这类平台写脚本）", () => {
    const { container } = renderSection([]);
    expect(container.querySelector("textarea")).not.toBeNull();
  });

  it("有内置变体且未选自定义：显示变体下拉，不显示脚本编辑器", () => {
    const { container } = renderSection([VARIANT]);
    expect(container.querySelector("button[role='combobox']")).not.toBeNull();
    expect(container.querySelector("textarea")).toBeNull();
  });

  it("有内置变体但选中自定义：显示脚本编辑器", () => {
    const { container } = renderSection([VARIANT], "", QUOTA_CUSTOM_VARIANT);
    expect(container.querySelector("textarea")).not.toBeNull();
  });

  it("自定义编辑器空内容时显示样例；已有内容显示原文", () => {
    const empty = renderSection([]);
    expect((empty.container.querySelector("[data-testid='quota-custom-script']") as HTMLTextAreaElement).value).toBe(QUOTA_SCRIPT_SAMPLE);
    const own = renderSection([], "return {}");
    expect((own.container.querySelector("[data-testid='quota-custom-script']") as HTMLTextAreaElement).value).toBe("return {}");
  });

  it("样例内容不算「已选自定义」：有变体 + 状态为样例 → 仍选变体，无编辑器", () => {
    const { container } = renderSection([VARIANT], QUOTA_SCRIPT_SAMPLE);
    expect(container.querySelector("[data-testid='quota-custom-script']")).toBeNull();
  });

  it("newapi：自定义模式无 user_id 输入；变体模式有", () => {
    const custom = renderSection([], "", QUOTA_CUSTOM_VARIANT, "newapi");
    expect(custom.container.querySelector("[data-testid='quota-newapi-user-id']")).toBeNull();
    const variant = renderSection([VARIANT], "", "default", "newapi");
    expect(variant.container.querySelector("[data-testid='quota-newapi-user-id']")).not.toBeNull();
  });
});

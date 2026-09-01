// 行为断言（区块渲染与否 / 自定义脚本编辑器出现与否），不按文案文字断言。
import { describe, it, expect, vi } from "vitest";
import { render } from "../../test/render";
import { QuotaScriptSection, QUOTA_CUSTOM_VARIANT } from "./formSections";
import type { QuotaScriptVariant } from "../../domains/platforms";

const t = ((key: string, fallback?: string) => fallback ?? key) as unknown as Parameters<typeof QuotaScriptSection>[0]["t"];

function renderSection(variants: QuotaScriptVariant[], customScript = "", variantId = "") {
  const onCustomScriptChange = vi.fn();
  const { container } = render(
    <QuotaScriptSection
      protocol="bailian_coding"
      variants={variants}
      variantId={variantId}
      onVariantChange={vi.fn()}
      customScript={customScript}
      onCustomScriptChange={onCustomScriptChange}
      requires={{}}
      onRequiresChange={vi.fn()}
      t={t}
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

describe("QuotaScriptSection", () => {
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
});

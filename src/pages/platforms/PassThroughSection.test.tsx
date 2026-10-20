// psfix-03 票 03：认证头预设补形态（Key / token / APIKey / API-KEY）。
// 覆盖：预设清单含新 4 项；下拉打开渲染全部预设；选中新预设落 header+template 正确；
// 新预设选中态双匹配判定（不落自定义输入框）。
import { beforeAll, describe, it, expect, vi } from "vitest";
import userEvent from "@testing-library/user-event";
import { render, screen } from "../../test/render";
import { PASS_THROUGH_HEADER_PRESETS, PassThroughSection } from "./formSections";
import type { PlatformEndpoint, PassthroughAuth, Protocol } from "../../services/api";

const t = ((key: string, fallback?: string) => fallback ?? key) as unknown as Parameters<typeof PassThroughSection>[0]["t"];

// jsdom 无 PointerEvent，Radix Select 需要 capture / scrollIntoView 垫片。
beforeAll(() => {
  if (typeof (window as { PointerEvent?: unknown }).PointerEvent === "undefined") {
    (window as { PointerEvent?: unknown }).PointerEvent = MouseEvent;
  }
  Object.assign(Element.prototype, {
    hasPointerCapture: () => false,
    setPointerCapture: () => {},
    releasePointerCapture: () => {},
    scrollIntoView: () => {},
  });
});

const EPS: PlatformEndpoint[] = [
  { protocol: "passthrough" as Protocol, base_url: "", client_type: "default", coding_plan: false },
];

function harness(auth: PassthroughAuth = { auth_header: "", auth_template: "" }) {
  const setAuth = vi.fn();
  const setEndpoints = vi.fn();
  const { container } = render(
    <PassThroughSection
      endpoints={EPS}
      setEndpoints={setEndpoints}
      apiKey=""
      setApiKey={vi.fn()}
      showKey={false}
      setShowKey={vi.fn()}
      auth={auth}
      setAuth={setAuth}
      t={t}
    />,
  );
  return { setAuth, setEndpoints, container };
}

const NEW_PRESETS = [
  { header: "Authorization", template: "Key {key}" },
  { header: "Authorization", template: "token {key}" },
  { header: "Authorization", template: "APIKey {key}" },
  { header: "Authorization", template: "API-KEY {key}" },
];

describe("PassThroughSection 认证头预设（psfix-03 票 03）", () => {
  it("预设清单含既有 4 项 + 新 4 项，id 唯一", () => {
    const ids = PASS_THROUGH_HEADER_PRESETS.map(p => p.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(PASS_THROUGH_HEADER_PRESETS.slice(0, 4)).toEqual([
      { id: "bearer", header: "Authorization", template: "Bearer {key}" },
      { id: "x-api-key", header: "x-api-key", template: "{key}" },
      { id: "api-key", header: "api-key", template: "{key}" },
      { id: "x-goog-api-key", header: "x-goog-api-key", template: "{key}" },
    ]);
    expect(PASS_THROUGH_HEADER_PRESETS.slice(4)).toEqual(NEW_PRESETS.map((p, i) => ({ ...p, id: ids[i + 4] })));
  });

  it("下拉渲染全部 8 个预设项", async () => {
    const user = userEvent.setup();
    harness();
    await user.click(screen.getByRole("combobox"));
    for (const p of PASS_THROUGH_HEADER_PRESETS) {
      expect(screen.getByRole("option", { name: `${p.header}: ${p.template}` })).not.toBeNull();
    }
  });

  it.each(NEW_PRESETS)("选中新预设 %s → setAuth 落对应 header+template", async (p) => {
    const user = userEvent.setup();
    const { setAuth } = harness();
    await user.click(screen.getByRole("combobox"));
    await user.click(screen.getByRole("option", { name: `${p.header}: ${p.template}` }));
    expect(setAuth).toHaveBeenCalledWith({ auth_header: p.header, auth_template: p.template });
  });

  it.each(NEW_PRESETS)("新预设值回显时命中预设（不显示自定义输入框）", (p) => {
    const { container } = harness({ auth_header: p.header, auth_template: p.template });
    // 自定义输入区（Header 名 / 值模板）仅在 selection === custom 时渲染；
    // 双匹配命中预设 → 不出现。
    expect(container.querySelector("input[placeholder='X-My-Auth']")).toBeNull();
    expect(container.querySelector("input[placeholder='Bearer {key}']")).toBeNull();
  });
});

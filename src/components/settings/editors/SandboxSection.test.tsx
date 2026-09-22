// 回归 2026-09-22：「禁止逃逸」开关点了存不进去。
//
// 这个开关的「开」= `allowUnsandboxedCommands: false`（该字段默认 true），而 `sync`
// 原先会把任何顶层 `false` 当成「与默认值相同」删掉，于是开关点完自己弹回去。
// 修法是给这一个 key 开例外（`SandboxSection.tsx:128` 的 FALSE_IS_MEANINGFUL）。
// Flutter 侧同一条断言在 `flutter/test/settings/parity05_editors_widget_test.dart`。
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "../../../test/render";
import { SandboxSectionInline } from "./SandboxSection";

describe("SandboxSection", () => {
  /** 「禁止逃逸」那一行的开关。按标签文案定位，不依赖 DOM 顺序。 */
  const noEscapeSwitch = () =>
    screen
      .getByText("settings.sandbox.noEscape")
      .closest("div")!
      .querySelector('[role="switch"]')!;

  it("「禁止逃逸」开关存得进去（allowUnsandboxedCommands: false 不被当默认值删掉）", () => {
    const updateField = vi.fn();
    render(
      <SandboxSectionInline sandboxValue={{ enabled: true }} updateField={updateField} />,
    );
    fireEvent.click(noEscapeSwitch());
    expect(updateField).toHaveBeenCalledWith(
      "sandbox",
      expect.objectContaining({ allowUnsandboxedCommands: false }),
    );
  });

  it("其余 boolean 关掉时仍然删字段（默认就是 false，不必存）", () => {
    const updateField = vi.fn();
    render(
      <SandboxSectionInline
        sandboxValue={{ enabled: true, failIfUnavailable: true }}
        updateField={updateField}
      />,
    );
    const sw = screen
      .getByText("settings.sandbox.failIfUnavailable")
      .closest("div")!
      .querySelector('[role="switch"]')!;
    fireEvent.click(sw);
    const saved = updateField.mock.calls.at(-1)![1] as Record<string, unknown>;
    expect(saved).not.toHaveProperty("failIfUnavailable");
  });
});

// ponytail: 改测行为（选中态/选择器出现与否/onSave 入参），禁按文案文字断言。
// 另一 agent 同时改本文件的文案 fallback 字面量 + 8 locale 值 → 本测试只用 id/结构断言，不用 getByText(文案)。
import { describe, it, expect, vi } from "vitest";
import { render, fireEvent } from "../../test/render";
import { WindowsEditModal } from "./WindowsEditModal";
import type { PeakWindow } from "../../domains/platforms/defaults";

// t 用简单 stub：有 fallback 返 fallback，无 fallback 返 key 本身（与 test/render.tsx 的空 resources 回退同构）。
const t = ((key: string, fallback?: string) => fallback ?? key) as unknown as Parameters<typeof WindowsEditModal>[0]["t"];

function renderModal(windows: PeakWindow[], onSave = vi.fn(), onClose = vi.fn()) {
  const utils = render(
    <WindowsEditModal
      open
      windows={windows}
      onSave={onSave}
      onClose={onClose}
      tzMode="utc"
      setTzMode={vi.fn()}
      t={t}
    />
  );
  return { ...utils, onSave, onClose };
}

/** 数字网格（每月几日 1-31）按钮：纯数字文本、无 title（周几按钮有 title，用于互斥区分）。 */
function monthButtons(container: HTMLElement) {
  return Array.from(container.querySelectorAll("button")).filter(
    (b) => /^\d{1,2}$/.test(b.textContent ?? "") && !b.hasAttribute("title")
  );
}

/** 周几 toggle 按钮：title 形如 platform.weekday_short.0（t stub 无 fallback 回退 key 本身）。 */
function weekdayButtons(container: HTMLElement) {
  return Array.from(container.querySelectorAll('button[title^="platform.weekday_short."]'));
}

describe("WindowsEditModal", () => {
  it("切「周几」: 七个星期 toggle 出现，选中态不弹回", () => {
    const { container } = renderModal([{ start_hour: 0, end_hour: 24, multiplier: 1 }]);
    const weekRadio = container.querySelector("#dim-0-week") as HTMLElement;
    fireEvent.click(weekRadio);
    expect(weekRadio).toHaveAttribute("data-state", "checked");
    expect(weekdayButtons(container)).toHaveLength(7);
    expect(monthButtons(container)).toHaveLength(0);
  });

  it("切回「每天」: 两个选择器收起, 保存后 days_of_week/days_of_month 均 undefined", () => {
    const { container, onSave } = renderModal([
      { start_hour: 0, end_hour: 24, multiplier: 1, days_of_week: [1, 2] },
    ]);
    const noneRadio = container.querySelector("#dim-0-none") as HTMLElement;
    fireEvent.click(noneRadio);
    expect(weekdayButtons(container)).toHaveLength(0);
    expect(monthButtons(container)).toHaveLength(0);

    fireEvent.click(container.querySelector("button.ripple") as HTMLElement);
    expect(onSave).toHaveBeenCalledTimes(1);
    const saved = onSave.mock.calls[0][0] as PeakWindow[];
    expect(saved[0].days_of_week).toBeUndefined();
    expect(saved[0].days_of_month).toBeUndefined();
  });

  it("选「周几」但一天不勾: 保存后不产生空数组脏数据, 仍是 undefined", () => {
    const { container, onSave } = renderModal([{ start_hour: 0, end_hour: 24, multiplier: 1 }]);
    fireEvent.click(container.querySelector("#dim-0-week") as HTMLElement);
    expect(weekdayButtons(container)).toHaveLength(7); // 选择器已露出、未勾选任何一天

    fireEvent.click(container.querySelector("button.ripple") as HTMLElement);
    expect(onSave).toHaveBeenCalledTimes(1);
    const saved = onSave.mock.calls[0][0] as PeakWindow[];
    expect(saved[0].days_of_week).toBeUndefined();
    expect(saved[0].days_of_month).toBeUndefined();
  });

  it("多窗口: 切其中一个的维度不影响其他窗口", () => {
    const { container } = renderModal([
      { start_hour: 0, end_hour: 24, multiplier: 1 },
      { start_hour: 0, end_hour: 24, multiplier: 1 },
    ]);
    fireEvent.click(container.querySelector("#dim-0-week") as HTMLElement);

    expect(container.querySelector("#dim-0-week")).toHaveAttribute("data-state", "checked");
    // window 1 仍是初始「无」选中态，未被 window 0 的切换影响
    expect(container.querySelector("#dim-1-none")).toHaveAttribute("data-state", "checked");
    expect(container.querySelector("#dim-1-week")).toHaveAttribute("data-state", "unchecked");
    // 只有 window 0 露出了周几选择器（7 个），不是 14 个（两个窗口都露出）
    expect(weekdayButtons(container)).toHaveLength(7);
  });
});

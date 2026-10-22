// ponytail: 改测行为（选中态/选择器出现与否/onSave 入参），禁按文案文字断言。
// 另一 agent 同时改本文件的文案 fallback 字面量 + 8 locale 值 → 本测试只用 id/结构断言，不用 getByText(文案)。
import { describe, it, expect, vi } from "vitest";
import { render, fireEvent } from "../../test/render";
import { WindowsEditModal } from "./WindowsEditModal";
import type { TimeWindow } from "../../domains/platforms/defaults";

// t 用简单 stub：有 fallback 返 fallback，无 fallback 返 key 本身（与 test/render.tsx 的空 resources 回退同构）。
const t = ((key: string, fallback?: string) => fallback ?? key) as unknown as Parameters<typeof WindowsEditModal>[0]["t"];

// Dialog 走 Radix Portal，内容渲染进 document.body，不在 RTL container 内 → 断言一律走 document.body。
function renderModal(windows: TimeWindow[], onSave = vi.fn(), onClose = vi.fn()) {
  const { rerender } = render(
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
  return { container: document.body, onSave, onClose, rerender };
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
    expect(weekRadio).toHaveAttribute("data-state", "on");
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
    const saved = onSave.mock.calls[0][0] as TimeWindow[];
    expect(saved[0].days_of_week).toBeUndefined();
    expect(saved[0].days_of_month).toBeUndefined();
  });

  it("选「周几」但一天不勾: 保存后不产生空数组脏数据, 仍是 undefined", () => {
    const { container, onSave } = renderModal([{ start_hour: 0, end_hour: 24, multiplier: 1 }]);
    fireEvent.click(container.querySelector("#dim-0-week") as HTMLElement);
    expect(weekdayButtons(container)).toHaveLength(7); // 选择器已露出、未勾选任何一天

    fireEvent.click(container.querySelector("button.ripple") as HTMLElement);
    expect(onSave).toHaveBeenCalledTimes(1);
    const saved = onSave.mock.calls[0][0] as TimeWindow[];
    expect(saved[0].days_of_week).toBeUndefined();
    expect(saved[0].days_of_month).toBeUndefined();
  });

  it("多窗口: 切其中一个的维度不影响其他窗口", () => {
    const { container } = renderModal([
      { start_hour: 0, end_hour: 24, multiplier: 1 },
      { start_hour: 0, end_hour: 24, multiplier: 1 },
    ]);
    fireEvent.click(container.querySelector("#dim-0-week") as HTMLElement);

    expect(container.querySelector("#dim-0-week")).toHaveAttribute("data-state", "on");
    // window 1 仍是初始「无」选中态，未被 window 0 的切换影响
    expect(container.querySelector("#dim-1-none")).toHaveAttribute("data-state", "on");
    expect(container.querySelector("#dim-1-week")).toHaveAttribute("data-state", "off");
    // 只有 window 0 露出了周几选择器（7 个），不是 14 个（两个窗口都露出）
    expect(weekdayButtons(container)).toHaveLength(7);
  });

  it("关闭再重开: uiDim 按 windows 数据重算, 周/月/都无三种形态选中态都对", () => {
    const windows: TimeWindow[] = [
      { start_hour: 0, end_hour: 24, multiplier: 1, days_of_week: [1, 2] },
      { start_hour: 0, end_hour: 24, multiplier: 1, days_of_month: [5] },
      { start_hour: 0, end_hour: 24, multiplier: 1 },
    ];
    const onSave = vi.fn();
    const onClose = vi.fn();
    const { container, rerender } = renderModal(windows, onSave, onClose);

    // 关闭
    rerender(
      <WindowsEditModal open={false} windows={windows} onSave={onSave} onClose={onClose}
        tzMode="utc" setTzMode={vi.fn()} t={t} />
    );
    // 重开：同一批 windows 数据不变，uiDim 应从数据重算出对应三态
    rerender(
      <WindowsEditModal open windows={windows} onSave={onSave} onClose={onClose}
        tzMode="utc" setTzMode={vi.fn()} t={t} />
    );

    expect(container.querySelector("#dim-0-week")).toHaveAttribute("data-state", "on");
    expect(container.querySelector("#dim-1-month")).toHaveAttribute("data-state", "on");
    expect(container.querySelector("#dim-2-none")).toHaveAttribute("data-state", "on");
  });

  it("删中间窗口: 剩余窗口的选中态各自不变（不因数组前移而错位）", () => {
    const windows: TimeWindow[] = [
      { start_hour: 0, end_hour: 24, multiplier: 1 },                    // widx0: none
      { start_hour: 0, end_hour: 24, multiplier: 1, days_of_week: [3] }, // widx1: week
      { start_hour: 0, end_hour: 24, multiplier: 1, days_of_month: [9] }, // widx2: month
    ];
    const { container } = renderModal(windows);

    // 删除 idx=1（原 week 窗口）—— 删除按钮以符号 "×" 标识（非文案，跨语言不变），结构上按序对应各窗口
    const removeButtons = Array.from(container.querySelectorAll("button")).filter(
      (b) => b.textContent === "×"
    );
    fireEvent.click(removeButtons[1]);

    // 剩余两窗口：原 widx0(none) 现仍是 dim-0-none，原 widx2(month) 现变成 dim-1-month
    expect(container.querySelector("#dim-0-none")).toHaveAttribute("data-state", "on");
    expect(container.querySelector("#dim-1-month")).toHaveAttribute("data-state", "on");
    // 不应错位成 dim-1-none 被选中（那是删除前 widx2 的位置套了 widx1 的选中态）
    expect(container.querySelector("#dim-1-none")).toHaveAttribute("data-state", "off");
    expect(monthButtons(container)).toHaveLength(31);
    expect(weekdayButtons(container)).toHaveLength(0);
  });
});

// ── TimeStepper 单挂：钳位 / 24:00 强制分=0 / 键入路径（review 2026-10-22 findings ②③④）──
import { TimeStepper } from "./WindowsEditModal";

function renderStepper(props: Partial<Parameters<typeof TimeStepper>[0]> = {}) {
  const onSet = vi.fn();
  // 查询范围限本 render 的 container：test/render 无自动 cleanup，
  // document.body 里残留前面 modal 测试的 stepper，全局查询会点错组件。
  const { container } = render(
    <TimeStepper
      suffix="止"
      hour={12}
      minute={0}
      maxHour={24}
      onSet={onSet}
      t={t}
      {...props}
    />
  );
  return { onSet, base: container };
}

/** 第二行箭头 = 向下（▼）：[小时-, 分钟-] */
function downArrows(container: ParentNode) {
  const downs = Array.from(container.querySelectorAll("button")).filter(b => b.textContent === "▼");
  return { hour: downs[0], minute: downs[1] };
}
function upArrows(container: ParentNode) {
  const ups = Array.from(container.querySelectorAll("button")).filter(b => b.textContent === "▲");
  return { hour: ups[0], minute: ups[1] };
}

describe("TimeStepper — 钳位与 24:00 语义", () => {
  it("止侧键入小时 24 → 分钟强制归 0（24:xx 只允许 24:00）", () => {
    const { onSet, base } = renderStepper({ hour: 23, minute: 30 });
    const hourInput = base.querySelectorAll("input")[0] as HTMLInputElement;
    fireEvent.change(hourInput, { target: { value: "24" } });
    expect(onSet).toHaveBeenCalledWith(24, 0);
  });

  it("止侧 24:00 点小时下箭头 → 23:00（分保持 0）", () => {
    const { onSet, base } = renderStepper({ hour: 24, minute: 0 });
    fireEvent.click(downArrows(base).hour!);
    expect(onSet).toHaveBeenCalledWith(23, 0);
  });

  it("起侧 23:55 分钟上箭头（+15）→ 钳到 23:59 不跨日", () => {
    const { onSet, base } = renderStepper({ hour: 23, minute: 55, maxHour: 23 });
    fireEvent.click(upArrows(base).minute!);
    expect(onSet).toHaveBeenCalledWith(23, 59);
  });

  it("键入任意分钟（非 15 倍数）可达 —— 22:37", () => {
    const { onSet, base } = renderStepper({ hour: 0, minute: 0, maxHour: 23 });
    const [h, m] = base.querySelectorAll("input");
    fireEvent.change(h as HTMLInputElement, { target: { value: "22" } });
    fireEvent.change(m as HTMLInputElement, { target: { value: "37" } });
    // stepper 是无状态受控件：分钟提交按当前 props（hour=0）算，父组件负责回灌后再组合成 22:37
    expect(onSet).toHaveBeenNthCalledWith(1, 22, 0);
    expect(onSet).toHaveBeenNthCalledWith(2, 0, 37);
  });

  it("值无变化不回调（0:00 点小时下箭头 no-op）", () => {
    const { onSet, base } = renderStepper({ hour: 0, minute: 0 });
    fireEvent.click(downArrows(base).hour!);
    expect(onSet).not.toHaveBeenCalled();
  });
});

describe("WindowsEditModal — 维度分段控件 ARIA 分组", () => {
  it("分段按钮的父容器带 radiogroup + aria-label（radio 必须有分组父级）", () => {
    renderModal([{ start_hour: 0, end_hour: 24, multiplier: 1.0 }]);
    const groups = document.body.querySelectorAll('[role="radiogroup"][aria-label]');
    expect(groups.length).toBeGreaterThan(0);
    expect(groups[groups.length - 1].querySelectorAll('[role="radio"]').length).toBe(3);
  });
});

describe("WindowsEditModal — 摘要行 scope/跨天", () => {
  it("每月几日：选 1 与 15 → 摘要含 \"1·15\"（数据拼接，非 i18n 译文）", () => {
    const { container } = renderModal([{ start_hour: 0, end_hour: 24, multiplier: 1.0 }]);
    fireEvent.click(container.querySelector("#dim-0-month") as HTMLElement);
    const days = monthButtons(container).filter(b => ["1", "15"].includes(b.textContent ?? ""));
    days.forEach(d => fireEvent.click(d));
    expect(container.textContent).toContain("1·15");
  });

  it("跨天窗口（止<起）→ 摘要出「次日」徽标", () => {
    const { container } = renderModal([{ start_hour: 22, end_hour: 6, multiplier: 1.0 }]);
    expect(container.textContent).toContain("次日");
  });
});

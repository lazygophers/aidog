// GroupEditPanel 就近测试：「最大尝试次数」输入限制 1–10（请求尝试-逐次重算 票 01）。
// 渲染真组件，选择器锚 testid / role，文案不入断言。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent } from "@testing-library/react";
import { render, screen } from "../../test/render";
import { EMPTY_EDIT } from "../../domains/groups";
import { GroupEditPanel } from "./GroupEditPanel";

// 子 hook 订阅 registry 事件会走 SSE 传输层；jsdom 无 EventSource，桩成空实现（不外呼）。
vi.stubGlobal("EventSource", class { onerror = null; addEventListener() {} removeEventListener() {} close() {} });

function setup() {
  const dispatchEdit = vi.fn();
  render(
    <GroupEditPanel
      edit={{ ...EMPTY_EDIT, name: "g", maxRetries: 4, target: { group: { id: 1, name: "g", group_key: "gk", extra: "", env_vars: [] }, platforms: [], model_mappings: [] } as never }}
      dispatchEdit={dispatchEdit}
      platforms={[]}
      t={((k: string) => k) as never}
      onCancel={vi.fn()}
      onSave={vi.fn()}
    />,
  );
  fireEvent.click(screen.getAllByRole("tab")[1]); // 「连接」tab
  return { dispatchEdit, input: screen.getByTestId("group-max-attempts") as HTMLInputElement };
}

describe("GroupEditPanel — 最大尝试次数输入限制", () => {
  beforeEach(() => vi.clearAllMocks());

  it("输入框范围属性为 1–10", () => {
    const { input } = setup();
    expect(input.min).toBe("1");
    expect(input.max).toBe("10");
    expect(input.value).toBe("4");
  });

  it.each([
    ["0", 1],
    ["-3", 1],
    ["", 1],
    ["11", 10],
    ["99", 10],
    ["5", 5],
    ["3.7", 4],
  ])("输入 %j → 写入 %i", (raw, want) => {
    const { dispatchEdit, input } = setup();
    fireEvent.change(input, { target: { value: raw } });
    expect(dispatchEdit).toHaveBeenLastCalledWith({ type: "patch", patch: { maxRetries: want } });
  });
});

describe("GroupEditPanel — 映射目标模型下拉含时段档模型", () => {
  it("目标平台只有时段档模型 → 渲染下拉而非无候选回退", () => {
    const plat = { id: 9, name: "P", platform_type: "openai", models: {}, status: "enabled",
      extra: JSON.stringify({ time_windows: [{ windows: [], models: { default: "w1" } }] }) } as never;
    render(
      <GroupEditPanel
        edit={{ ...EMPTY_EDIT, name: "g", mappings: [{ source_model: "s", target_platform_id: 9, target_model: "" }], target: { group: { id: 1, name: "g", group_key: "gk", extra: "", is_default: false } } } as never}
        dispatchEdit={vi.fn()}
        platforms={[plat]}
        t={((k: string) => k) as never}
        onCancel={vi.fn()}
        onSave={vi.fn()}
      />,
    );
    expect(screen.queryByTestId("mapping-target-model-select")).not.toBeNull();
  });
});

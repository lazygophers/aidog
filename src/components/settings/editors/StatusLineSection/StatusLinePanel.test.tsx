import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { StatusLinePanel } from "./StatusLinePanel";
import { SEGMENT_DEF_MAP } from "../../statusline-gen";

vi.mock("../../../../services/api", () => ({ statuslineApi: {} }));

const t = ((_k: string, d?: string) => d ?? _k) as never;

// 官方新增字段对应的 segment，设置页添加菜单必须能选到。
const NEW_TYPES = [
  "spend-limit", "prompt-cache", "fast-mode", "worktree-path",
  "worktree-original-cwd", "pr-kind", "agent-type", "task-description", "task-effort",
];

function open(scriptType: "statusline" | "subagent") {
  render(
    <StatusLinePanel
      config={{ [scriptType === "statusline" ? "_aidog_statusline" : "_aidog_subagent_statusline"]: { enabled: true, mode: "builtin" } }}
      updateField={() => {}}
      scriptType={scriptType}
      t={t}
    />,
  );
  fireEvent.click(screen.getByTestId("sl-add-toggle"));
}

describe("StatusLinePanel 添加菜单", () => {
  it("既有 segment 仍可选（基线）", () => {
    open("statusline");
    expect(screen.getByTestId("sl-add-pr-state")).toBeTruthy();
  });

  it.each(NEW_TYPES)("新 segment %s 可在添加菜单选到且有定义", (type) => {
    open("statusline");
    expect(screen.getByTestId(`sl-add-${type}`)).toBeTruthy();
    expect(SEGMENT_DEF_MAP.get(type as never)).toBeTruthy();
  });
});

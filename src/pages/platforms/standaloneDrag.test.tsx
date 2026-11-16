import { describe, it, expect } from "vitest";
import { createPortal } from "react-dom";
import { render, fireEvent, screen } from "@testing-library/react";
import { shouldSkipStandaloneDrag } from "./usePlatformsState";

// 复刻 PlatformListView 未分组行：外层 div onPointerDown + 行内 portal 到 body 的下拉菜单项。
function Row({ onStart }: { onStart: () => void }) {
  return (
    <div
      data-testid="wrap"
      onPointerDown={(e) => { if (!shouldSkipStandaloneDrag(e)) onStart(); }}
    >
      <span data-testid="row-body">row</span>
      <button data-testid="row-btn">b</button>
      {createPortal(<div role="menuitem" data-testid="menu-delete">删除平台</div>, document.body)}
    </div>
  );
}

describe("shouldSkipStandaloneDrag", () => {
  it("portal 菜单项的 pointerdown 不启动拖拽", () => {
    let started = 0;
    render(<Row onStart={() => started++} />);
    fireEvent.pointerDown(screen.getByTestId("menu-delete"));
    expect(started).toBe(0);
  });
  it("行本体 pointerdown 仍启动拖拽，按钮让位", () => {
    let started = 0;
    render(<Row onStart={() => started++} />);
    fireEvent.pointerDown(screen.getByTestId("row-body"));
    expect(started).toBe(1);
    fireEvent.pointerDown(screen.getByTestId("row-btn"));
    expect(started).toBe(1);
  });
});

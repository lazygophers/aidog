// G3 票 04 / 旧票 03：删除平台确认浮层就近测试。
import { describe, it, expect, vi } from "vitest";
import { fireEvent } from "@testing-library/react";
import { render } from "../../test/render";
import { DeletePlatformConfirm } from "./DeletePlatformConfirm";

describe("DeletePlatformConfirm — 删除确认（票 04）", () => {
  it("open 态渲染三段文案 + 取消/删除两键；确认触发 onConfirm", () => {
    const onConfirm = vi.fn();
    const onCancel = vi.fn();
    const { rerender } = render(
      <DeletePlatformConfirm open name="glm_coding" onConfirm={onConfirm} onCancel={onCancel} />,
    );
    const dialog = document.querySelector('[id="g3-delete-confirm"]');
    expect(dialog?.getAttribute("data-state")).toBe("open");
    expect(dialog?.textContent).toContain("platform.deleteConfirm.body");
    expect(dialog?.textContent).toContain("platform.deleteConfirm.irreversible");
    fireEvent.click(document.getElementById("g3-delete-ok")!);
    expect(onConfirm).toHaveBeenCalledTimes(1);
    fireEvent.click(document.getElementById("g3-delete-cancel")!);
    expect(onCancel).toHaveBeenCalledTimes(1);
    rerender(<DeletePlatformConfirm open={false} onConfirm={onConfirm} onCancel={onCancel} />);
  });

  it("busy 态删除键禁用", () => {
    render(<DeletePlatformConfirm open busy onConfirm={vi.fn()} onCancel={vi.fn()} />);
    const ok = document.getElementById("g3-delete-ok")!;
    expect(ok.getAttribute("data-busy")).toBe("yes");
    expect((ok as HTMLButtonElement).disabled).toBe(true);
  });
});

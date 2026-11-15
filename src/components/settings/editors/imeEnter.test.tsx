// 输入法组字期间的 Enter（确认候选词）不得触发添加；普通 Enter 照常添加。
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "../../../test/render";
import { StringListEditor } from "./_shared";
import { SandboxSectionInline } from "./SandboxSection";

const composing = { key: "Enter", isComposing: true };
// WKWebView 确认键到达时 isComposing 已复位，只剩 keyCode 229
const safari = { key: "Enter", keyCode: 229 };
const plain = { key: "Enter", keyCode: 13 };

describe("输入法 Enter 防误提交", () => {
  it("StringListEditor：组字中的 Enter 不添加，普通 Enter 添加", () => {
    const onChange = vi.fn();
    render(<StringListEditor items={[]} onChange={onChange} addLabel="add" />);
    const input = screen.getByTestId("string-list-draft");
    fireEvent.change(input, { target: { value: "你好" } });
    fireEvent.keyDown(input, composing);
    fireEvent.keyDown(input, safari);
    expect(onChange).not.toHaveBeenCalled();
    fireEvent.keyDown(input, plain);
    expect(onChange).toHaveBeenCalledWith(["你好"]);
  });

  it("SandboxSection 路径列表：组字中的 Enter 不添加，普通 Enter 添加", () => {
    const updateField = vi.fn();
    render(<SandboxSectionInline sandboxValue={{ enabled: true }} updateField={updateField} />);
    const input = screen.getAllByTestId("path-list-draft")[0];
    fireEvent.change(input, { target: { value: "/tmp/x" } });
    fireEvent.keyDown(input, composing);
    fireEvent.keyDown(input, safari);
    expect(updateField).not.toHaveBeenCalled();
    fireEvent.keyDown(input, plain);
    expect(updateField).toHaveBeenCalled();
  });
});

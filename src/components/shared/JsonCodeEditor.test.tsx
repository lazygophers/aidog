// JsonCodeEditor 的外部行为：可编辑态有格式化按钮且能把压行 JSON 展开；只读态不给编辑入口。
// 不断言 CodeMirror 内部 DOM（jsdom 下虚拟滚动只渲染视口内的行，断言内容等于测实现细节）。

import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { JsonCodeEditor } from "./JsonCodeEditor";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

describe("JsonCodeEditor", () => {
  it("可编辑时给出格式化按钮，点击后把压成一行的 JSON 展开成缩进格式", async () => {
    const onChange = vi.fn();
    render(<JsonCodeEditor value={'{"a":1,"b":[2,3]}'} onChange={onChange} />);

    const btn = screen.getByRole("button", { name: "jsonEditor.format" });
    await userEvent.click(btn);

    expect(onChange).toHaveBeenCalledWith(JSON.stringify({ a: 1, b: [2, 3] }, null, 2));
  });

  it("非法 JSON 点格式化不抛错、也不回调（错误由行内标红承担）", async () => {
    const onChange = vi.fn();
    render(<JsonCodeEditor value={'{"a":'} onChange={onChange} />);

    await userEvent.click(screen.getByRole("button", { name: "jsonEditor.format" }));

    expect(onChange).not.toHaveBeenCalled();
  });

  it("只读态（不传 onChange）不渲染格式化按钮", () => {
    render(<JsonCodeEditor value={'{"a":1}'} />);
    expect(screen.queryByRole("button", { name: "jsonEditor.format" })).toBeNull();
  });

  it("父层传入的 error 展示在编辑器下方", () => {
    render(<JsonCodeEditor value="{}" onChange={() => {}} error="保存失败：字段 x 非法" />);
    expect(screen.getByText("保存失败：字段 x 非法")).toBeTruthy();
  });
});

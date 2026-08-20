import { describe, it, expect, vi } from "vitest";
import { render, screen } from "../../test/render";
import userEvent from "@testing-library/user-event";
import { FilterDropdown } from "./FilterDropdown";

const OPTIONS = [
  { value: "openai", label: "OpenAI" },
  { value: "glm", label: "智谱 GLM" },
  { value: "kimi", label: "Kimi" },
];

const setup = (value = "", onChange = vi.fn()) => {
  const user = userEvent.setup();
  render(
    <FilterDropdown
      width={200}
      value={value}
      onChange={onChange}
      allLabel="全部平台"
      searchPlaceholder="搜索"
      options={OPTIONS}
      emptyLabel="无匹配"
    />,
  );
  return { user, onChange, trigger: screen.getAllByRole("button")[0] };
};

describe("FilterDropdown", () => {
  it("value 为空显示 allLabel，非空显示对应 option 的 label", () => {
    const { trigger } = setup("");
    expect(trigger).toHaveTextContent("全部平台");
  });

  it("未知 value 回落到 allLabel（选项列表变化后残留旧值不显示裸 id）", () => {
    const { trigger } = setup("已删除的平台");
    expect(trigger).toHaveTextContent("全部平台");
  });

  it("已知 value 显示其 label", () => {
    const { trigger } = setup("glm");
    expect(trigger).toHaveTextContent("智谱 GLM");
  });

  it("打开后列出全部选项", async () => {
    const { user, trigger } = setup();
    await user.click(trigger);
    for (const o of OPTIONS) expect(await screen.findByText(o.label)).toBeInTheDocument();
  });

  it("搜索按 label 大小写不敏感过滤，无匹配显示 emptyLabel", async () => {
    const { user, trigger } = setup();
    await user.click(trigger);
    const input = await screen.findByPlaceholderText("搜索");

    await user.type(input, "kim");
    expect(screen.getByText("Kimi")).toBeInTheDocument();
    expect(screen.queryByText("OpenAI")).not.toBeInTheDocument();

    await user.clear(input);
    await user.type(input, "OPENAI");
    expect(screen.getByText("OpenAI")).toBeInTheDocument();

    await user.clear(input);
    await user.type(input, "zzz");
    expect(screen.getByText("无匹配")).toBeInTheDocument();
  });

  it("空白搜索串不过滤", async () => {
    const { user, trigger } = setup();
    await user.click(trigger);
    await user.type(await screen.findByPlaceholderText("搜索"), "   ");
    expect(screen.getByText("Kimi")).toBeInTheDocument();
  });

  it("选中选项回调其 value 并关闭浮层", async () => {
    const onChange = vi.fn();
    const { user, trigger } = setup("", onChange);
    await user.click(trigger);
    await user.click(await screen.findByText("智谱 GLM"));
    expect(onChange).toHaveBeenCalledWith("glm");
    await vi.waitFor(() => expect(screen.queryByPlaceholderText("搜索")).not.toBeInTheDocument());
  });

  it("选「全部」回调空串", async () => {
    const onChange = vi.fn();
    const { user, trigger } = setup("glm", onChange);
    await user.click(trigger);
    // trigger 与浮层里的「全部平台」同名，取浮层内那个（后出现的）
    const all = await screen.findAllByText("全部平台");
    await user.click(all[all.length - 1]);
    expect(onChange).toHaveBeenCalledWith("");
  });

  it("重开时搜索框已被清空", async () => {
    const { user, trigger } = setup();
    await user.click(trigger);
    await user.type(await screen.findByPlaceholderText("搜索"), "kim");
    await user.keyboard("{Escape}");
    await user.click(trigger);
    expect(await screen.findByPlaceholderText("搜索")).toHaveValue("");
  });
});

// ClientFilterSelect（issue #66）：真组件渲染 + data-testid 断言。
// 多选切换、显示名映射（词表码 / ua: 前缀）、搜索过滤。
import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { ClientFilterSelect, toggleClient } from "./ClientFilterSelect";

// i18next 直通：t(key, fallback) 返回 fallback（词表 key 在测试环境无翻译表）。
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (_k: string, fb: string) => fb }),
}));

describe("toggleClient", () => {
  it("勾选追加、取消移除、清空即全部", () => {
    expect(toggleClient([], "codex")).toEqual(["codex"]);
    expect(toggleClient(["codex"], "claude_code")).toEqual(["codex", "claude_code"]);
    expect(toggleClient(["codex", "claude_code"], "codex")).toEqual(["claude_code"]);
    expect(toggleClient(["codex"], "codex")).toEqual([]);
  });
});

describe("ClientFilterSelect", () => {
  const clients = ["claude_code", "codex", "ua:opencode", "other"];

  it("触发器默认显示全部，展开后列出全部选项并带显示名", () => {
    render(<ClientFilterSelect clients={clients} selected={[]} onChange={() => {}} />);
    const trigger = screen.getByTestId("stats-client-filter");
    expect(trigger).toHaveTextContent("All Clients");
    fireEvent.click(trigger);
    expect(screen.getByTestId("stats-client-opt-claude_code")).toHaveTextContent("claude_code");
    // ua:<产品名> 剥前缀显示产品名（未识别客户端照原样归类）
    expect(screen.getByTestId("stats-client-opt-ua:opencode")).toHaveTextContent("opencode");
    expect(screen.getByTestId("stats-client-opt-other")).toHaveTextContent("other");
  });

  it("勾选回调带该客户端码，选中态高亮", () => {
    const onChange = vi.fn();
    render(<ClientFilterSelect clients={clients} selected={[]} onChange={onChange} />);
    fireEvent.click(screen.getByTestId("stats-client-filter"));
    fireEvent.click(screen.getByTestId("stats-client-opt-codex"));
    expect(onChange).toHaveBeenCalledWith(["codex"]);
    // 已选中渲染：勾选态样式 + 触发器显示选中名
    render(
      <ClientFilterSelect clients={clients} selected={["codex"]} onChange={() => {}} />,
    );
    expect(screen.getAllByTestId("stats-client-filter")[1]).toHaveTextContent("codex");
  });

  it("搜索过滤选项", () => {
    render(<ClientFilterSelect clients={clients} selected={[]} onChange={() => {}} />);
    fireEvent.click(screen.getByTestId("stats-client-filter"));
    fireEvent.change(screen.getByTestId("stats-client-search"), { target: { value: "codex" } });
    expect(screen.getByTestId("stats-client-opt-codex")).toBeInTheDocument();
    expect(screen.queryByTestId("stats-client-opt-claude_code")).not.toBeInTheDocument();
  });
});

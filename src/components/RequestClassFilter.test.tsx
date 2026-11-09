// RequestClassFilter：选项集 + onChange 值约定（"" = 全部，"unknown" = 未知，其余原样）。
import { describe, it, expect, vi } from "vitest";
import { fireEvent } from "@testing-library/react";
import { render, screen } from "../test/render";

// Radix Select 在 jsdom 无法开面板：换成原生 select 壳（同 DisplayCurrencySection.test）。
vi.mock("@/components/ui/select", () => ({
  Select: ({ value, onValueChange, children }: { value: string; onValueChange: (v: string) => void; children: React.ReactNode }) => (
    <select data-testid="request-class-filter" value={value} onChange={(e) => onValueChange(e.target.value)}>{children}</select>
  ),
  SelectTrigger: () => null,
  SelectValue: () => null,
  SelectContent: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  SelectItem: ({ value, children }: { value: string; children: React.ReactNode }) => <option value={value}>{children}</option>,
}));

import { RequestClassFilter } from "./RequestClassFilter";

describe("RequestClassFilter", () => {
  it("选项：全部 + 6 个已知 + 未知", () => {
    render(<RequestClassFilter value="" onChange={() => {}} />);
    const vals = [...screen.getByTestId("request-class-filter").querySelectorAll("option")].map(o => o.getAttribute("value"));
    expect(vals).toEqual(["all", "main", "subagent", "workflow", "compaction", "auxiliary", "side_reply", "unknown"]);
  });

  it("value='' 显示为 all；选 all 回调 ''", () => {
    const onChange = vi.fn();
    render(<RequestClassFilter value="" onChange={onChange} />);
    const sel = screen.getByTestId("request-class-filter") as HTMLSelectElement;
    expect(sel.value).toBe("all");
    fireEvent.change(sel, { target: { value: "main" } });
    expect(onChange).toHaveBeenLastCalledWith("main");
    fireEvent.change(sel, { target: { value: "unknown" } });
    expect(onChange).toHaveBeenLastCalledWith("unknown");
    fireEvent.change(sel, { target: { value: "all" } });
    expect(onChange).toHaveBeenLastCalledWith("");
  });
});

import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { StatusLineOptions, pickStatusLineOptions } from "./StatusLineOptions";

const t = (_k: string, d: string) => d;

describe("pickStatusLineOptions", () => {
  it("只取用户设过且合法的项", () => {
    expect(pickStatusLineOptions({ enabled: true })).toEqual({});
    expect(
      pickStatusLineOptions({ padding: 0, refreshInterval: 5, hideVimModeIndicator: false }),
    ).toEqual({ padding: 0, refreshInterval: 5, hideVimModeIndicator: false });
    expect(
      pickStatusLineOptions({ padding: -1, refreshInterval: 0, hideVimModeIndicator: "x" }),
    ).toEqual({});
    expect(pickStatusLineOptions({ padding: 1.5, refreshInterval: null })).toEqual({});
  });
});

describe("StatusLineOptions", () => {
  it("未设置时输入框为空、开关关闭", () => {
    render(<StatusLineOptions stored={{}} onChange={() => {}} t={t} />);
    expect((screen.getByTestId("sl-opt-padding") as HTMLInputElement).value).toBe("");
    expect((screen.getByTestId("sl-opt-refresh") as HTMLInputElement).value).toBe("");
    expect(screen.getByTestId("sl-opt-hidevim").getAttribute("data-state")).toBe("off");
  });

  it("回显已设值", () => {
    render(
      <StatusLineOptions
        stored={{ padding: 2, refreshInterval: 10, hideVimModeIndicator: true }}
        onChange={() => {}}
        t={t}
      />,
    );
    expect((screen.getByTestId("sl-opt-padding") as HTMLInputElement).value).toBe("2");
    expect((screen.getByTestId("sl-opt-refresh") as HTMLInputElement).value).toBe("10");
    expect(screen.getByTestId("sl-opt-hidevim").getAttribute("data-state")).toBe("on");
  });

  it("编辑 / 清空 / 切换向上回调", () => {
    const onChange = vi.fn();
    render(<StatusLineOptions stored={{ refreshInterval: 10 }} onChange={onChange} t={t} />);
    fireEvent.change(screen.getByTestId("sl-opt-padding"), { target: { value: "3" } });
    expect(onChange).toHaveBeenLastCalledWith("padding", 3);
    fireEvent.change(screen.getByTestId("sl-opt-refresh"), { target: { value: "" } });
    expect(onChange).toHaveBeenLastCalledWith("refreshInterval", undefined);
    fireEvent.click(screen.getByTestId("sl-opt-hidevim"));
    expect(onChange).toHaveBeenLastCalledWith("hideVimModeIndicator", true);
  });
});

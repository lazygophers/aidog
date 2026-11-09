// 「试一试」面板：真组件 + data-testid，mock API。

import { describe, it, expect, vi } from "vitest";
import { render, screen, waitFor } from "../../test/render";
import userEvent from "@testing-library/user-event";
import { MiddlewareTryPanel } from "./MiddlewareTryPanel";

const tryRun = vi.hoisted(() => vi.fn());
vi.mock("../../services/api/scheduling", () => ({
  middlewareApi: { tryRun: (...a: unknown[]) => tryRun(...a) },
}));

describe("MiddlewareTryPanel", () => {
  it("提交样例文本后展示替换后文本 / 命中规则 / 还原后文本", async () => {
    tryRun.mockResolvedValue({ masked: "MASKED", hits: ["rule-a", "rule-b"], restored: "RESTORED" });
    render(<MiddlewareTryPanel />);
    await userEvent.type(screen.getByTestId("mw-try-input"), "hello");
    await userEvent.click(screen.getByTestId("mw-try-run"));
    await waitFor(() => expect(screen.getByTestId("mw-try-masked").textContent).toBe("MASKED"));
    expect(tryRun).toHaveBeenCalledWith("hello");
    expect(screen.getByTestId("mw-try-hits").textContent).toContain("rule-a");
    expect(screen.getByTestId("mw-try-hits").textContent).toContain("rule-b");
    expect(screen.getByTestId("mw-try-restored").textContent).toBe("RESTORED");
  });

  it("无命中时显示空态，不展示规则名", async () => {
    tryRun.mockResolvedValue({ masked: "x", hits: [], restored: "x" });
    render(<MiddlewareTryPanel />);
    await userEvent.type(screen.getByTestId("mw-try-input"), "x");
    await userEvent.click(screen.getByTestId("mw-try-run"));
    await waitFor(() => expect(screen.getByTestId("mw-try-hits-empty")).toBeTruthy());
  });
});

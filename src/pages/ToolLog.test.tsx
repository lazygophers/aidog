// 工具日志页：列表渲染 / 筛选透传 / 行点击拉详情（引擎明细 + 正文）/ 保留天数读写 / 分页。
// 断言用 i18n key + data-testid（render 隔离实例回传 key 本身）。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { screen, fireEvent, waitFor } from "@testing-library/react";
import { render } from "../test/render";
import { ToolLog } from "./ToolLog";

const invokeMock = vi.fn();
vi.mock("../services/transport", () => ({ invoke: (...a: unknown[]) => invokeMock(...a) }));

const ROW = {
  id: 7, created_at: 1759000000000, tool: "web_search", trigger: "gateway", group_name: "team-a",
  platform: "glm", query: "rust async", result_count: 3,
  engines: JSON.stringify([
    { engine: "bing", count: 3, latency_ms: 120, error: null },
    { engine: "google", count: 0, latency_ms: 15000, error: "timeout" },
  ]),
  duration_ms: 15120, success: true, error: "", result: "",
};
const FAILED = { ...ROW, id: 6, trigger: "mcp", success: false, error: "all engines failed", query: "" };

function setup(rows = [ROW, FAILED], total = rows.length) {
  invokeMock.mockImplementation((cmd: string) => {
    if (cmd === "tool_log_list") return Promise.resolve({ total, rows });
    if (cmd === "tool_log_get") return Promise.resolve({ ...ROW, result: "1. Rust Lang https://www.rust-lang.org/" });
    if (cmd === "tool_log_retention_get") return Promise.resolve(30);
    return Promise.resolve(null);
  });
}

beforeEach(() => { invokeMock.mockReset(); });

describe("ToolLog", () => {
  it("渲染列表行；点击行拉详情，展示各引擎明细与结果正文", async () => {
    setup();
    render(<ToolLog />);
    expect(await screen.findByTestId("tool-log-row-7")).toBeTruthy();
    expect(screen.getByTestId("tool-log-row-6")).toBeTruthy();
    expect(invokeMock).toHaveBeenCalledWith("tool_log_list", { filter: {}, limit: 50, offset: 0 });

    fireEvent.click(screen.getByTestId("tool-log-row-7"));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("tool_log_get", { id: 7 }));
    expect(await screen.findByTestId("tool-log-detail-7")).toBeTruthy();
    expect(screen.getByTestId("tool-log-engine-bing").textContent).toContain("3");
    expect(screen.getByTestId("tool-log-engine-google").textContent).toContain("timeout");
    expect(screen.getByTestId("tool-log-result-7").textContent).toContain("rust-lang.org");

    fireEvent.click(screen.getByTestId("tool-log-row-7"));
    expect(screen.queryByTestId("tool-log-detail-7")).toBeNull();
  });

  it("正文为空（隐私开关关）时显占位 key", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "tool_log_list") return Promise.resolve({ total: 1, rows: [ROW] });
      if (cmd === "tool_log_get") return Promise.resolve({ ...ROW, result: "" });
      if (cmd === "tool_log_retention_get") return Promise.resolve(30);
      return Promise.resolve(null);
    });
    render(<ToolLog />);
    fireEvent.click(await screen.findByTestId("tool-log-row-7"));
    expect((await screen.findByTestId("tool-log-result-7")).textContent).toBe("toolLog.resultEmpty");
  });

  it("筛选变化带着 filter 重新拉列表并回到第一页", async () => {
    setup();
    render(<ToolLog />);
    await screen.findByTestId("tool-log-row-7");
    fireEvent.change(screen.getByTestId("tool-log-filter-trigger"), { target: { value: "mcp" } });
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("tool_log_list", { filter: { trigger: "mcp" }, limit: 50, offset: 0 }),
    );
    fireEvent.change(screen.getByTestId("tool-log-filter-tool"), { target: { value: "web_fetch" } });
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("tool_log_list", { filter: { trigger: "mcp", tool: "web_fetch" }, limit: 50, offset: 0 }),
    );
    fireEvent.change(screen.getByTestId("tool-log-filter-group"), { target: { value: "team-a" } });
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("tool_log_list", {
        filter: { trigger: "mcp", tool: "web_fetch", group_name: "team-a" }, limit: 50, offset: 0,
      }),
    );
  });

  it("总数超过一页时可翻页（offset 递增）", async () => {
    setup([ROW], 120);
    render(<ToolLog />);
    await screen.findByTestId("tool-log-row-7");
    fireEvent.click(screen.getByTestId("tool-log-next"));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("tool_log_list", { filter: {}, limit: 50, offset: 50 }),
    );
  });

  it("保留天数：读出当前值，修改后保存调用 retentionSet", async () => {
    setup();
    render(<ToolLog />);
    const input = (await screen.findByTestId("tool-log-retention-input")) as HTMLInputElement;
    await waitFor(() => expect(input.value).toBe("30"));
    fireEvent.change(input, { target: { value: "7" } });
    fireEvent.click(screen.getByTestId("tool-log-retention-save"));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("tool_log_retention_set", { days: 7 }));
  });

  it("空列表显示占位", async () => {
    setup([], 0);
    render(<ToolLog />);
    expect(await screen.findByTestId("tool-log-empty")).toBeTruthy();
  });
});

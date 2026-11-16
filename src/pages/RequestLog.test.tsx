// 请求记录页：无 平台/模型/token 列；表头与行单元格数一致（含 UA 列）；平台筛选仍在。锚 data-testid / DOM 结构，禁文案断言。
import { describe, it, expect, vi } from "vitest";
import { render, screen, waitFor } from "../test/render";
import { Table, TableBody } from "@/components/ui/table";
import { LogRow } from "./Logs/primitives";
import type { ProxyLogSummary } from "../services/api";

const rows = [
  { id: "r1", kind: "test", group_key: "[test]", platform_id: 7, status_code: 200, duration_ms: 42, user_agent: "ua-1", created_at: 1700000000000 },
  { id: "r2", kind: "quota", group_key: "[quota]", platform_id: 8, status_code: 500, duration_ms: 9, user_agent: "", created_at: 1700000001000 },
];

vi.mock("../services/api", async (orig) => ({
  ...(await orig<typeof import("../services/api")>()),
  requestLogApi: {
    list: vi.fn(async () => rows),
    count: vi.fn(async () => rows.length),
    get: vi.fn(async () => null),
  },
  platformApi: { list: vi.fn(async () => [{ id: 7, name: "P7" }, { id: 8, name: "P8" }]) },
  onProxyLogUpdated: () => () => {},
}));

import { RequestLog } from "./RequestLog";

describe("RequestLog 页", () => {
  it("表头与行单元格数一致，且没有 平台/模型/token 列", async () => {
    const { container } = render(<RequestLog />);
    await waitFor(() => expect(container.querySelectorAll("tbody tr").length).toBe(2));
    const ths = container.querySelectorAll("thead th").length;
    container.querySelectorAll("tbody tr").forEach((tr) => expect(tr.querySelectorAll("td").length).toBe(ths));
    // 时间 / 分组 / UA / 状态 / 耗时 / 操作
    expect(ths).toBe(6);
    expect(screen.queryByTestId("log-row-input-tokens")).toBeNull();
    expect(screen.queryByTestId("log-row-output-tokens")).toBeNull();
    expect(screen.getAllByTestId("request-log-row-ua")[0].textContent).toBe("ua-1");
  });

  it("平台筛选下拉仍在", async () => {
    render(<RequestLog />);
    await waitFor(() => expect(screen.getByTestId("request-log-filter-platform")).not.toBeNull());
  });
});

describe("代理日志 LogRow 不受影响", () => {
  it("仍显示模型与 token 列", () => {
    const log = {
      id: "p1", group_key: "g", model: "m1", actual_model: "m2", source_protocol: "anthropic",
      target_protocol: "openai", platform_id: 7, status_code: 200, duration_ms: 42,
      input_tokens: 11, output_tokens: 22, cache_tokens: 0, is_stream: false, retry_count: 0,
      blocked_by: "", blocked_reason: "", client: "", user_agent: "", request_url: "", up_bytes: 0, down_bytes: 0, created_at: 1,
    } as ProxyLogSummary;
    render(
      <Table><TableBody>
        <LogRow log={log} platformName="P" groupName="G" onOpen={() => {}} onCopy={() => {}} t={((k: string, d?: string) => d ?? k) as never} />
      </TableBody></Table>,
    );
    expect(screen.getByTestId("log-row-input-tokens").textContent).toBe("11");
    expect(screen.getByTestId("log-row-output-tokens").textContent).toBe("22");
  });
});

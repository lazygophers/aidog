// LogRow 隧道行（代理日志-MITM隧道 票 02）：目标地址 / 未解密标记 / 字节数 / 空 token 格。
// 锚 data-testid，禁文案断言。
import { describe, it, expect } from "vitest";
import { render, screen } from "../../test/render";
import { Table, TableBody } from "@/components/ui/table";
import { LogRow } from "./primitives";
import type { ProxyLogSummary } from "../../services/api";

const base = {
  id: "r1", group_key: "g", model: "", actual_model: "", source_protocol: "http-connect",
  target_protocol: "http-connect", platform_id: 7, status_code: 200, duration_ms: 42,
  input_tokens: 0, output_tokens: 0, cache_tokens: 0, is_stream: false, retry_count: 0,
  blocked_by: "", blocked_reason: "mitm_opaque", client: "", user_agent: "",
  request_url: "api.anthropic.com:443", up_bytes: 1536, down_bytes: 2048, created_at: 1,
} as ProxyLogSummary;

function row(log: ProxyLogSummary) {
  return render(
    <Table><TableBody>
      <LogRow log={log} platformName="P" groupName="G" onOpen={() => {}} onCopy={() => {}} t={((k: string, d?: string) => d ?? k) as never} />
    </TableBody></Table>,
  );
}

describe("LogRow 隧道行", () => {
  it("显示目标地址、未解密标记、字节数；token 格留空不显示 0 或 -", () => {
    row(base);
    expect(screen.getByTestId("log-row-tunnel-host").textContent).toBe("api.anthropic.com:443");
    expect(screen.getByTestId("log-row-opaque-badge")).not.toBeNull();
    expect(screen.getByTestId("log-row-bytes").textContent).toContain("1.5 KB");
    expect(screen.getByTestId("log-row-bytes").textContent).toContain("2.0 KB");
    expect(screen.getByTestId("log-row-input-tokens").textContent).toBe("");
    expect(screen.getByTestId("log-row-output-tokens").textContent).toBe("");
  });

  it("非 mitm_opaque 的隧道行无未解密标记", () => {
    row({ ...base, blocked_reason: "" });
    expect(screen.queryByTestId("log-row-opaque-badge")).toBeNull();
    expect(screen.getByTestId("log-row-tunnel-host")).not.toBeNull();
  });

  it("普通请求行不渲染隧道元素", () => {
    row({ ...base, source_protocol: "anthropic", blocked_reason: "", model: "m" });
    expect(screen.queryByTestId("log-row-tunnel-host")).toBeNull();
    expect(screen.queryByTestId("log-row-bytes")).toBeNull();
  });
});

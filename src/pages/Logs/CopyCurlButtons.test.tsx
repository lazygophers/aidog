// 票 passthrough-log-fixes/04：日志详情复制 curl。
// buildCurl 纯函数单测 + CopyCurlButtons 组件测试（断言经 mock writeText 落进剪贴板的生成串）。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "../../test/render";
import { writeText } from "../../services/platform";

const writeTextMock = vi.mocked(writeText);

vi.mock("../../services/platform", () => ({
  writeText: vi.fn(async () => {}),
}));

import { buildCurl, SENSITIVE_AUTH_HEADER_NAMES } from "./curl";
import { CopyCurlButtons } from "./CopyCurlButtons";

function detailFixture(extra: Partial<Record<string, unknown>> = {}) {
  return {
    id: "req-1",
    request_url: "http://127.0.0.1:9890/v1/messages",
    upstream_request_url: "https://api.example.com/v1/chat/completions",
    request_headers: JSON.stringify({
      "content-type": "application/json",
      authorization: "Bearer sk-user-key",
      "user-agent": "claude-code/1.0",
    }),
    request_body: JSON.stringify({ model: "gpt-x", messages: [{ role: "user", content: "hi\nthere" }] }),
    upstream_request_headers: JSON.stringify({
      "content-type": "application/json",
      "x-api-key": "sk-up-key",
      "user-agent": "aidog/1.0",
    }),
    upstream_request_body: JSON.stringify({ model: "up-model", messages: [] }),
    response_body: "",
    user_response_headers: "{}",
    user_response_body: "",
    upstream_response_headers: "{}",
    ...extra,
  } as any;
}

describe("buildCurl", () => {
  it("生成串含 -X method / URL / 占位 / body", () => {
    const s = buildCurl({
      method: "POST",
      url: "https://api.example.com/v1/messages",
      headers: { authorization: "Bearer sk-abc", "content-type": "application/json" },
      body: { model: "m" },
    });
    expect(s).toContain("-X POST");
    expect(s).toContain("'https://api.example.com/v1/messages'");
    expect(s).toContain("Bearer {key}");
    expect(s).not.toContain("sk-abc");
    expect(s).toContain("--data");
    expect(s).toContain('"model":"m"');
  });

  it("非鉴权头原样、鉴权头 scheme 保留；无 scheme 裸 key 直接 {key}", () => {
    const s = buildCurl({
      url: "https://x.example.com",
      headers: { "user-agent": "curl/8.0", "x-api-key": "sk-bare" },
    });
    expect(s).toContain("'user-agent: curl/8.0'");
    expect(s).toContain("'x-api-key: {key}'");
  });

  it("六头名单语义与 Rust is_sensitive_log_header 镜像", () => {
    expect([...SENSITIVE_AUTH_HEADER_NAMES].sort()).toEqual(
      ["api-key", "authorization", "cookie", "set-cookie", "x-api-key", "x-goog-api-key"].sort(),
    );
  });

  it("多行 JSON body 压成合法单行字符串字面量", () => {
    const s = buildCurl({
      url: "https://x.example.com",
      headers: {},
      body: JSON.stringify({ a: 1, b: "x\ny" }, null, 2),
    });
    expect(s).not.toMatch(/--data '\{\n/);
    expect(s).toContain(`--data '{"a":1,"b":"x\\ny"}'`);
  });

  it("空 body 不带 --data", () => {
    const s = buildCurl({ url: "https://x.example.com", headers: {}, body: "" });
    expect(s).not.toContain("--data");
  });

  it("值内单引号转义为 '\\''", () => {
    const s = buildCurl({ url: "https://x.example.com", headers: { "x-note": "it's" } });
    expect(s).toContain(`'x-note: it'\\''s'`);
  });

  it("method 缺省 POST", () => {
    const s = buildCurl({ url: "https://x.example.com", headers: {} });
    expect(s).toContain("-X POST");
  });
});

describe("CopyCurlButtons", () => {
  beforeEach(() => {
    writeTextMock.mockClear();
    writeTextMock.mockResolvedValue(undefined);
  });

  it("两个按钮渲染，title 用 i18n key", () => {
    render(<CopyCurlButtons detail={detailFixture()} />);
    expect(screen.getByTitle("logs.copyUserCurl")).toBeTruthy();
    expect(screen.getByTitle("logs.copyUpstreamCurl")).toBeTruthy();
  });

  it("用户侧：生成串含 method/URL/鉴权占位/body，非鉴权头原样", async () => {
    render(<CopyCurlButtons detail={detailFixture()} />);
    fireEvent.click(screen.getByTitle("logs.copyUserCurl"));
    await waitFor(() => expect(writeTextMock).toHaveBeenCalled());
    const s = writeTextMock.mock.calls[0][0] as string;
    expect(s).toContain("-X POST");
    expect(s).toContain("http://127.0.0.1:9890/v1/messages");
    expect(s).toContain("'authorization: Bearer {key}'");
    expect(s).toContain("'user-agent: claude-code/1.0'");
    expect(s).not.toContain("sk-user-key");
    expect(s).toContain('--data \'{"model":"gpt-x"');
  });

  it("上游侧：URL/头/body 取上游字段", async () => {
    render(<CopyCurlButtons detail={detailFixture()} />);
    fireEvent.click(screen.getByTitle("logs.copyUpstreamCurl"));
    await waitFor(() => expect(writeTextMock).toHaveBeenCalled());
    const s = writeTextMock.mock.calls[0][0] as string;
    expect(s).toContain("https://api.example.com/v1/chat/completions");
    expect(s).toContain("'x-api-key: {key}'");
    expect(s).toContain("'user-agent: aidog/1.0'");
    expect(s).toContain('--data \'{"model":"up-model"');
  });

  it("空 body 不带 --data（上游 body 缺失时）", async () => {
    render(
      <CopyCurlButtons
        detail={detailFixture({ upstream_request_body: "" })}
      />,
    );
    fireEvent.click(screen.getByTitle("logs.copyUpstreamCurl"));
    await waitFor(() => expect(writeTextMock).toHaveBeenCalled());
    const s = writeTextMock.mock.calls[0][0] as string;
    expect(s).not.toContain("--data");
  });
});

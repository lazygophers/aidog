// 分组「最后一次请求」徽标：成功 / 失败带原因 / 无请求 / 点击跳日志。选择器锚 testid + data-state。
import { describe, it, expect, vi } from "vitest";
import { fireEvent } from "@testing-library/react";
import { render, screen } from "../../test/render";
import type { GroupLastRequest as Last } from "../../services/api";
import { GroupLastRequest } from "./GroupLastRequest";

const base: Last = { id: "log-1", status_code: 200, created_at: Date.now() - 5000, duration_ms: 12, blocked_reason: "", error: "" };

describe("GroupLastRequest", () => {
  it("成功态：data-state=success，无原因节点", () => {
    render(<GroupLastRequest last={base} />);
    expect(screen.getByTestId("group-last-request").getAttribute("data-state")).toBe("success");
    expect(screen.queryByTestId("group-last-request-reason")).toBeNull();
    expect(screen.getByTestId("group-last-request-code").textContent).toBe("200");
  });

  it("失败态：带上游错误摘要", () => {
    render(<GroupLastRequest last={{ ...base, status_code: 502, error: "upstream boom" }} />);
    expect(screen.getByTestId("group-last-request").getAttribute("data-state")).toBe("failed");
    expect(screen.getByTestId("group-last-request-reason").getAttribute("title")).toBe("upstream boom");
  });

  it("失败态：blocked_reason 优先于 error", () => {
    render(<GroupLastRequest last={{ ...base, status_code: 503, blocked_reason: "rate_limit", error: "x" }} />);
    expect(screen.getByTestId("group-last-request-reason").getAttribute("title")).toBe("rate_limit");
  });

  it("无请求态", () => {
    render(<GroupLastRequest last={undefined} />);
    expect(screen.getByTestId("group-last-request").getAttribute("data-state")).toBe("none");
  });

  it("点击带 log id 回调", () => {
    const onOpen = vi.fn();
    render(<GroupLastRequest last={base} onOpen={onOpen} />);
    fireEvent.click(screen.getByTestId("group-last-request"));
    expect(onOpen).toHaveBeenCalledWith("log-1");
  });
});

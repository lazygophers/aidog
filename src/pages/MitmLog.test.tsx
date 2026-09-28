// 任务 3 单测：观测页列表渲染 + 行点击按需拉详情（body 有值可查看、无值显占位）。
// 断言用 i18n key（test/render 隔离实例回传 key 本身）。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { screen, fireEvent, waitFor } from "@testing-library/react";
import { render } from "../test/render";
import { MitmLog } from "./MitmLog";

// invoke 按 cmd 名分发（bypassList / mitm_bypass_detail）。
const invokeMock = vi.fn();
vi.mock("../services/transport", () => ({ invoke: (...a: unknown[]) => invokeMock(...a) }));

const ROW = {
  id: 7, group_name: "g1", host: "api.anthropic.com", path: "/api/oauth/profile",
  status_code: 200, req_bytes: 10, resp_bytes: 20, decrypted: true, created_at: 1759000000000,
};

beforeEach(() => { invokeMock.mockReset(); });

describe("MitmLog", () => {
  it("空列表显示占位；有行渲染 host+path；点击行拉详情并展示 body", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "mitm_bypass_list") return Promise.resolve([ROW]);
      if (cmd === "mitm_bypass_detail")
        return Promise.resolve({ request_body: '{"a":1}', response_body: "" });
      return Promise.resolve([]);
    });
    render(<MitmLog />);
    expect(await screen.findByText(/api\.anthropic\.com\/api\/oauth\/profile/)).toBeTruthy();

    // 点击行 → 详情按需拉取，body 有值展示（prettify 后多行），空值显占位 key。
    fireEvent.click(screen.getByText(/api\.anthropic\.com\/api\/oauth\/profile/));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("mitm_bypass_detail", { id: 7 }));
    expect(await screen.findByText(/"a": 1/)).toBeTruthy();
    expect(screen.getAllByText("page.mitmBodyEmpty").length).toBe(1);

    // 再点收起，详情行消失。
    fireEvent.click(screen.getByText(/api\.anthropic\.com\/api\/oauth\/profile/));
    expect(screen.queryByText(/"a": 1/)).toBeNull();
  });

  it("列表空时显 bypassEmpty 占位，不发详情请求", async () => {
    invokeMock.mockResolvedValue([]);
    render(<MitmLog />);
    expect(await screen.findByText("stats.bypassEmpty")).toBeTruthy();
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });
});

import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "../../test/render";
import type { ApiCallLog } from "../../services/api";

const h = vi.hoisted(() => ({ callLog: vi.fn() }));
vi.mock("../../services/transport", () => ({ invoke: vi.fn(), listen: vi.fn(async () => () => {}) }));
vi.mock("../../services/api", () => ({ kernelApi: { callLog: h.callLog } }));

import { ApiCallLogSection } from "./ApiCallLogSection";

const row = (id: number, o: Partial<ApiCallLog> = {}): ApiCallLog => ({
  id, created_at: 1_700_000_000_000, command: "platform_list", status_code: 200,
  success: true, duration_ms: 12, user_agent: "curl/8", source: "third_party", ...o,
});

describe("ApiCallLogSection", () => {
  beforeEach(() => h.callLog.mockReset());

  it("renders rows with success/failed state and source", async () => {
    h.callLog.mockResolvedValue([
      row(2, { command: "platform_delete", status_code: 401, success: false, source: "settings_test" }),
      row(1),
    ]);
    render(<ApiCallLogSection />);
    await waitFor(() => expect(screen.getByTestId("api-call-log")).toHaveAttribute("data-state", "ready"));
    expect(screen.getByTestId("api-call-log-row-1")).toHaveAttribute("data-state", "ok");
    expect(screen.getByTestId("api-call-log-row-1")).toHaveAttribute("data-source", "third_party");
    expect(screen.getByTestId("api-call-log-row-2")).toHaveAttribute("data-state", "failed");
    expect(screen.getByTestId("api-call-log-row-2")).toHaveAttribute("data-source", "settings_test");
    expect(screen.getByTestId("api-call-log-row-2")).toHaveTextContent("platform_delete");
  });

  it("shows empty state", async () => {
    h.callLog.mockResolvedValue([]);
    render(<ApiCallLogSection />);
    await waitFor(() => expect(screen.getByTestId("api-call-log")).toHaveAttribute("data-state", "empty"));
    expect(screen.getByTestId("api-call-log-empty")).toBeInTheDocument();
  });

  it("shows error state on failure and refresh reloads", async () => {
    h.callLog.mockRejectedValueOnce("boom").mockResolvedValueOnce([row(7)]);
    render(<ApiCallLogSection />);
    await waitFor(() => expect(screen.getByTestId("api-call-log")).toHaveAttribute("data-state", "error"));
    fireEvent.click(screen.getByTestId("api-call-log-refresh"));
    await waitFor(() => expect(screen.getByTestId("api-call-log")).toHaveAttribute("data-state", "ready"));
    expect(h.callLog).toHaveBeenCalledTimes(2);
  });
});

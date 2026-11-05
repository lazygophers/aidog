import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "../../test/render";
import { ApiTestPanel } from "./ApiTestPanel";

const h = vi.hoisted(() => ({ kernel: vi.fn(), send: vi.fn() }));
vi.mock("../../services/transport", () => ({ invoke: vi.fn(), listen: vi.fn(async () => () => {}) }));
vi.mock("../../services/api", () => ({
  kernelApi: { getSettings: h.kernel, testSend: h.send },
}));
vi.mock("../../generated/apiCatalog.json", () => ({
  default: {
    commands: [
      { name: "platform_list", category: "platform", kind: "readonly", exportsSecret: false, desktopOnly: false, params: [], returns: "Vec<Platform>" },
      { name: "platform_delete", category: "platform", kind: "mutating", exportsSecret: false, desktopOnly: false, params: [{ name: "id", type: "u64" }], returns: "()" },
      { name: "db_master_key_reveal", category: "system", kind: null, exportsSecret: false, desktopOnly: true, params: [], returns: "String" },
    ],
  },
}));

const ready = (state = "ready") => waitFor(() => expect(screen.getByTestId("api-test")).toHaveAttribute("data-state", state));
const pick = (name: string) => fireEvent.change(screen.getByTestId("api-test-command"), { target: { value: name } });

describe("ApiTestPanel", () => {
  beforeEach(() => {
    h.kernel.mockReset().mockResolvedValue({ api_enabled: true, auth_token: "" });
    h.send.mockReset().mockResolvedValue({ status: 200, duration_ms: 12, body: "[]" });
  });

  it("readonly command sends directly, no confirm", async () => {
    render(<ApiTestPanel />);
    await ready();
    pick("platform_list");
    fireEvent.click(screen.getByTestId("api-test-send"));
    await waitFor(() => expect(screen.getByTestId("api-test-result")).toHaveAttribute("data-state", "ok"));
    expect(h.send).toHaveBeenCalledWith("platform_list", "{}");
    expect(screen.queryByTestId("api-test-confirm")).toBeNull();
    expect(screen.getByTestId("api-test-status")).toHaveTextContent("200");
    expect(screen.getByTestId("api-test-body")).toHaveTextContent("[]");
  });

  it("mutating command asks first; cancel does not send, confirm sends", async () => {
    render(<ApiTestPanel />);
    await ready();
    pick("platform_delete");
    fireEvent.change(screen.getByTestId("api-test-params"), { target: { value: '{"id":7}' } });
    fireEvent.click(screen.getByTestId("api-test-send"));
    const dlg = await screen.findByTestId("api-test-confirm");
    expect(dlg).toHaveTextContent("platform_delete");
    expect(dlg).toHaveTextContent('{"id":7}');
    fireEvent.click(screen.getByTestId("api-test-confirm-cancel"));
    await waitFor(() => expect(screen.queryByTestId("api-test-confirm")).toBeNull());
    expect(h.send).not.toHaveBeenCalled();

    fireEvent.click(screen.getByTestId("api-test-send"));
    fireEvent.click(await screen.findByTestId("api-test-confirm-ok"));
    await waitFor(() => expect(h.send).toHaveBeenCalledWith("platform_delete", '{"id":7}'));
  });

  it("selecting a command prefills the params skeleton once", async () => {
    render(<ApiTestPanel />);
    await ready();
    pick("platform_delete");
    expect(screen.getByTestId("api-test-params")).toHaveValue('{"id":0}');
  });

  it("desktop-only commands are not selectable", async () => {
    render(<ApiTestPanel />);
    await ready();
    expect(screen.queryByRole("option", { name: "db_master_key_reveal" })).toBeNull();
  });

  it("management API switched off: hint shown, send disabled", async () => {
    h.kernel.mockResolvedValue({ api_enabled: false, auth_token: "" });
    render(<ApiTestPanel />);
    await ready("disabled");
    expect(screen.getByTestId("api-test-disabled-hint")).toBeInTheDocument();
    expect(screen.getByTestId("api-test-send")).toBeDisabled();
  });

  it("proxy not running: hint instead of result", async () => {
    h.send.mockRejectedValue("proxy_unreachable");
    render(<ApiTestPanel />);
    await ready();
    pick("platform_list");
    fireEvent.click(screen.getByTestId("api-test-send"));
    await waitFor(() => expect(screen.getByTestId("api-test-result")).toHaveAttribute("data-state", "proxy-down"));
  });

  it("non-2xx is shown as error state with the real status", async () => {
    h.send.mockResolvedValue({ status: 404, duration_ms: 3, body: '{"error":"x"}' });
    render(<ApiTestPanel />);
    await ready();
    pick("platform_list");
    fireEvent.click(screen.getByTestId("api-test-send"));
    await waitFor(() => expect(screen.getByTestId("api-test-result")).toHaveAttribute("data-state", "error"));
    expect(screen.getByTestId("api-test-status")).toHaveTextContent("404");
  });
});

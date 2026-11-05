import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "../../test/render";
import { ApiCatalogSection } from "./ApiCatalogSection";

const h = vi.hoisted(() => ({ kernel: vi.fn(), proxy: vi.fn() }));
vi.mock("../../services/transport", () => ({ invoke: vi.fn(), listen: vi.fn(async () => () => {}) }));
vi.mock("../../services/api", () => ({
  kernelApi: { getSettings: h.kernel },
  proxyApi: { getSettings: h.proxy },
}));
vi.mock("../../generated/apiCatalog.json", () => ({
  default: {
    commands: [
      { name: "platform_list", category: "platform", kind: "readonly", exportsSecret: false, desktopOnly: false, params: [], returns: "Vec<Platform>" },
      { name: "platform_delete", category: "platform", kind: "mutating", exportsSecret: false, desktopOnly: false, params: [{ name: "id", type: "u64" }], returns: "()" },
      { name: "backup_export", category: "backup", kind: "mutating", exportsSecret: true, desktopOnly: false, params: [{ name: "passphrase", type: "Option<String>" }], returns: "String" },
      { name: "db_master_key_reveal", category: "system", kind: null, exportsSecret: false, desktopOnly: true, params: [], returns: "String" },
    ],
  },
}));

const setup = (port = 9890, token = "") => {
  h.kernel.mockResolvedValue({ api_enabled: true, auth_token: token });
  h.proxy.mockResolvedValue({ port });
};
const ready = () => waitFor(() => expect(screen.getByTestId("api-catalog")).toHaveAttribute("data-port", expect.stringMatching(/^\d+$/)));

describe("ApiCatalogSection", () => {
  beforeEach(() => { h.kernel.mockReset(); h.proxy.mockReset(); });

  it("lists commands with kind / secret / desktop-only markers", async () => {
    setup();
    render(<ApiCatalogSection />);
    await ready();
    expect(screen.getByTestId("api-cmd-platform_list")).toHaveAttribute("data-kind", "readonly");
    expect(screen.getByTestId("api-cmd-platform_delete")).toHaveAttribute("data-kind", "mutating");
    expect(screen.getByTestId("api-cmd-backup_export")).toHaveAttribute("data-secret", "true");
    expect(screen.getByTestId("api-cmd-db_master_key_reveal")).toHaveAttribute("data-kind", "desktop");
    expect(screen.getByTestId("api-cmd-platform_delete")).toHaveTextContent("id: u64");
    expect(screen.getByTestId("api-cmd-platform_list")).toHaveTextContent("Vec<Platform>");
  });

  it("search filters by name, category select filters by category", async () => {
    setup();
    render(<ApiCatalogSection />);
    await ready();
    fireEvent.change(screen.getByTestId("api-catalog-search"), { target: { value: "delete" } });
    expect(screen.queryByTestId("api-cmd-platform_list")).toBeNull();
    expect(screen.getByTestId("api-cmd-platform_delete")).toBeInTheDocument();
    fireEvent.change(screen.getByTestId("api-catalog-search"), { target: { value: "zzz" } });
    expect(screen.getByTestId("api-catalog")).toHaveAttribute("data-state", "empty");
    fireEvent.change(screen.getByTestId("api-catalog-search"), { target: { value: "" } });
    fireEvent.change(screen.getByTestId("api-catalog-category"), { target: { value: "backup" } });
    expect(screen.queryByTestId("api-cmd-platform_list")).toBeNull();
    expect(screen.getByTestId("api-cmd-backup_export")).toBeInTheDocument();
  });

  it("curl uses the live proxy port; Bearer placeholder only when credential set (never the secret)", async () => {
    setup(12345, "");
    const { rerender } = render(<ApiCatalogSection refreshKey={0} />);
    await waitFor(() => expect(screen.getByTestId("api-catalog")).toHaveAttribute("data-port", "12345"));
    const plain = screen.getByTestId("api-cmd-curl-platform_delete").textContent!;
    expect(plain).toContain("http://127.0.0.1:12345/api/rpc/platform_delete");
    expect(plain).not.toContain("Authorization");
    expect(plain).toContain(`-d '{"id":0}'`);

    setup(23456, "s3cret");
    rerender(<ApiCatalogSection refreshKey={1} />);
    await waitFor(() => expect(screen.getByTestId("api-catalog")).toHaveAttribute("data-port", "23456"));
    const authed = screen.getByTestId("api-cmd-curl-platform_delete").textContent!;
    expect(authed).toContain("http://127.0.0.1:23456/api/rpc/platform_delete");
    expect(authed).toContain("Authorization: Bearer <api-credential>");
    expect(authed).not.toContain("s3cret");
  });

  it("lists Local API and forward endpoints with auth curl", async () => {
    setup(9890);
    render(<ApiCatalogSection />);
    await ready();
    expect(screen.getByTestId("api-local-group-info")).toHaveTextContent("/api/group-info");
    expect(screen.getByTestId("api-local-curl-notify").textContent).toContain("Bearer <group-name>");
    expect(screen.getByTestId("api-forward-curl-chat").textContent).toContain("http://127.0.0.1:9890/v1/chat/completions");
    expect(screen.getByTestId("api-forward-models")).toBeInTheDocument();
  });
});

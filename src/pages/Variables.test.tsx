import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "../test/render";

const api = vi.hoisted(() => ({ get: vi.fn(), set: vi.fn(), syncGroupSettings: vi.fn(), saveCodex: vi.fn() }));
vi.mock("../services/api", () => ({ settingsApi: { get: api.get, set: api.set }, configApi: { syncGroupSettings: api.syncGroupSettings } }));
vi.mock("../services/api/variables", () => ({ variablesApi: { saveCodex: api.saveCodex } }));

import { Variables } from "./Variables";

beforeEach(() => {
  vi.clearAllMocks();
  api.get.mockImplementation((_scope, key) => Promise.resolve(key === "claude_code" ? { env: { EXISTING: "keep" } } : null));
  api.set.mockResolvedValue(undefined);
  api.syncGroupSettings.mockResolvedValue([]);
  api.saveCodex.mockResolvedValue([]);
});

describe("Variables", () => {
  it("saves Claude env into global Claude Code config and syncs groups", async () => {
    render(<Variables />);
    await screen.findByRole("textbox", { name: "variables.name" });
    fireEvent.click(screen.getByTestId("variable-add"));
    fireEvent.change(screen.getAllByRole("textbox", { name: "variables.name" })[1], { target: { value: "FEATURE_FLAG" } });
    fireEvent.change(screen.getAllByRole("textbox", { name: "variables.value" })[1], { target: { value: "enabled" } });
    fireEvent.click(screen.getAllByTestId("variable-claude")[1]);
    fireEvent.click(screen.getByTestId("variables-save"));
    await waitFor(() => expect(api.set).toHaveBeenCalledWith("global", "claude_code", {
      env: { EXISTING: "keep", FEATURE_FLAG: "enabled" },
    }));
    expect(api.syncGroupSettings).toHaveBeenCalledOnce();
  });

  it("saves Codex-selected variables through the Codex writer", async () => {
    render(<Variables />);
    await screen.findByRole("textbox", { name: "variables.name" });
    fireEvent.click(screen.getByTestId("variable-add"));
    fireEvent.change(screen.getAllByRole("textbox", { name: "variables.name" })[1], { target: { value: "FEATURE_FLAG" } });
    fireEvent.change(screen.getAllByRole("textbox", { name: "variables.value" })[1], { target: { value: "enabled" } });
    fireEvent.click(screen.getAllByTestId("variable-codex")[1]);
    fireEvent.click(screen.getByTestId("variables-save"));
    await waitFor(() => expect(api.saveCodex).toHaveBeenCalledWith([
      { key: "EXISTING", value: "keep", claude: true, codex: false },
      { key: "FEATURE_FLAG", value: "enabled", claude: false, codex: true },
    ]));
  });
});

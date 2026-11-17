import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "../test/render";

const api = vi.hoisted(() => ({ get: vi.fn(), save: vi.fn() }));
vi.mock("../services/api", () => ({ settingsApi: { get: api.get } }));
vi.mock("../services/api/variables", () => ({ variablesApi: { save: api.save } }));

import { Variables } from "./Variables";

beforeEach(() => {
  vi.clearAllMocks();
  api.get.mockImplementation((_scope, key) =>
    Promise.resolve(key === "extension_variables" ? [{ key: "EXISTING", value: "keep" }] : null),
  );
  api.save.mockResolvedValue(undefined);
});

describe("Variables", () => {
  it("renders the independently managed list without target switches and saves via variablesApi.save", async () => {
    render(<Variables />);
    await screen.findByRole("textbox", { name: "variables.name" });
    // 变量独立管理：不再渲染 Claude Code / Codex 目标开关，两边同时生效。
    expect(screen.queryByTestId("variable-claude")).toBeNull();
    expect(screen.queryByTestId("variable-codex")).toBeNull();
    fireEvent.click(screen.getByTestId("variable-add"));
    fireEvent.change(screen.getAllByRole("textbox", { name: "variables.name" })[1], { target: { value: "FEATURE_FLAG" } });
    fireEvent.change(screen.getAllByRole("textbox", { name: "variables.value" })[1], { target: { value: "enabled" } });
    fireEvent.click(screen.getByTestId("variables-save"));
    await waitFor(() => expect(api.save).toHaveBeenCalledWith([
      { key: "EXISTING", value: "keep" },
      { key: "FEATURE_FLAG", value: "enabled" },
    ]));
  });

  it("edits a same-name entry in place: save sends the overridden value to both targets", async () => {
    render(<Variables />);
    const name = await screen.findByRole("textbox", { name: "variables.name" });
    const value = screen.getAllByRole("textbox", { name: "variables.value" })[0];
    fireEvent.change(name, { target: { value: "EXISTING" } });
    fireEvent.change(value, { target: { value: "overridden" } });
    fireEvent.click(screen.getByTestId("variables-save"));
    await waitFor(() => expect(api.save).toHaveBeenCalledWith([
      { key: "EXISTING", value: "overridden" },
    ]));
  });

  it("reports backend errors", async () => {
    api.save.mockRejectedValue(new Error("invalid environment variable: BAD"));
    render(<Variables />);
    await screen.findByRole("textbox", { name: "variables.name" });
    fireEvent.click(screen.getByTestId("variables-save"));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("invalid environment variable"));
  });
});

import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor } from "../test/render";
import { Settings } from "./Settings";
import { requestNavigation } from "../utils/navGuard";

const api = vi.hoisted(() => ({ get: vi.fn(), set: vi.fn(), syncGroupSettings: vi.fn() }));
vi.mock("../services/api", () => ({
  settingsApi: api,
  configApi: api,
  claudeSettingsImportApi: { readDefault: vi.fn() },
}));
vi.mock("../services/claude-settings-schema", () => ({ SECTIONS: [], RECOMMENDED_CONFIG: { model: "default" } }));
vi.mock("../components/shared/JsonCodeEditor", () => ({
  JsonCodeEditor: ({ value, onChange, error }: { value: string; onChange: (value: string) => void; error?: string }) => (
    <><textarea aria-label="JSON" value={value} onChange={(e) => onChange(e.target.value)} />{error && <p role="alert">{error}</p>}</>
  ),
}));

beforeEach(() => {
  vi.clearAllMocks();
  api.get.mockResolvedValue({ model: "saved" });
  api.set.mockResolvedValue(undefined);
  api.syncGroupSettings.mockResolvedValue(undefined);
  const observer = class { observe() {} disconnect() {} unobserve() {} };
  vi.stubGlobal("ResizeObserver", observer);
  vi.stubGlobal("IntersectionObserver", observer);
});

const clickMode = (mode: "gui" | "json") => fireEvent.click(screen.getByRole("button", { name: `settings.${mode}Mode` }));
const save = () => screen.getByRole("button", { name: "action.save" });
async function loadedJson() {
  render(<Settings />);
  await waitFor(() => expect(api.get).toHaveBeenCalled());
  await act(async () => {});
  clickMode("json");
  expect(screen.getByRole("textbox", { name: "JSON" })).toHaveValue(JSON.stringify({ model: "saved" }, null, 2));
}
function edit(value: string) {
  fireEvent.change(screen.getByRole("textbox", { name: "JSON" }), { target: { value } });
}

describe("Settings draft modes", () => {
  it("preserves JSON changes through GUI and back", async () => {
    await loadedJson();
    edit('{"model":"draft"}');
    clickMode("gui");
    expect(save()).toBeEnabled();
    clickMode("json");
    expect(JSON.parse((screen.getByRole("textbox", { name: "JSON" }) as HTMLTextAreaElement).value)).toEqual({ model: "draft" });
  });

  it("clicking the current JSON tab does not reset the draft", async () => {
    await loadedJson();
    edit('{"model":"draft"}');
    clickMode("json");
    expect(screen.getByRole("textbox", { name: "JSON" })).toHaveValue('{"model":"draft"}');
  });

  it.each(["", "{", '{"model":}', '{"model":"draft",}'])("keeps invalid JSON dirty and blocks GUI switch: %s", async (value) => {
    await loadedJson();
    edit(value);
    expect(save()).toBeEnabled();
    clickMode("gui");
    expect(screen.getByRole("textbox", { name: "JSON" })).toHaveValue(value);
    expect(save()).toBeEnabled();
    expect(api.set).not.toHaveBeenCalled();
  });

  it("formatting-only changes are clean; fixing invalid JSON restores clean state", async () => {
    await loadedJson();
    edit("{");
    expect(save()).toBeEnabled();
    edit(' { "model" : "saved" } ');
    expect(save()).toBeDisabled();
  });

  it("cannot edit or save before initialization; navigation can leave", async () => {
    let resolve!: (value: unknown) => void;
    api.get.mockReturnValue(new Promise((done) => { resolve = done; }));
    const { unmount } = render(<Settings />);
    expect(screen.getByRole("button", { name: "settings.jsonMode" })).toBeInTheDocument();
    expect(save()).toBeDisabled();
    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    expect(api.set).not.toHaveBeenCalled();
    const proceed = vi.fn();
    requestNavigation(proceed);
    expect(proceed).toHaveBeenCalledOnce();
    unmount();
    await act(async () => resolve({ model: "late" }));
    expect(api.set).not.toHaveBeenCalled();
  });
});

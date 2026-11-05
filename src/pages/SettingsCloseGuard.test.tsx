import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor } from "../test/render";
import { Settings } from "./Settings";
import { useCloseGuard } from "../utils/closeGuard";

// 真 Settings + 真 closeGuard + 真 navGuard：只 mock Tauri 窗口 / 事件 API 与后端 api。
const api = vi.hoisted(() => ({ get: vi.fn(), set: vi.fn(), syncGroupSettings: vi.fn() }));
vi.mock("../services/api", () => ({
  settingsApi: api,
  configApi: api,
  settingsPresetApi: { list: vi.fn().mockResolvedValue([]) },
  claudeSettingsImportApi: { readDefault: vi.fn(), outputStyles: vi.fn().mockResolvedValue([]) },
}));
vi.mock("../services/claude-settings-schema", () => ({ SECTIONS: [] }));
vi.mock("../components/shared/JsonCodeEditor", () => ({
  JsonCodeEditor: ({ value, onChange }: { value: string; onChange: (v: string) => void }) => (
    <textarea aria-label="JSON" value={value} onChange={(e) => onChange(e.target.value)} />
  ),
}));

let closeHandler: ((e: { preventDefault: () => void }) => void) | null = null;
const destroy = vi.fn().mockResolvedValue(undefined);
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    onCloseRequested: async (h: typeof closeHandler) => {
      closeHandler = h;
      return () => {};
    },
    destroy,
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({ emit: vi.fn().mockResolvedValue(undefined) }));

function Page() {
  useCloseGuard();
  return <Settings />;
}

beforeEach(() => {
  vi.clearAllMocks();
  closeHandler = null;
  api.get.mockResolvedValue({ model: "saved" });
  api.set.mockResolvedValue(undefined);
  api.syncGroupSettings.mockResolvedValue(undefined);
  const observer = class { observe() {} disconnect() {} unobserve() {} };
  vi.stubGlobal("ResizeObserver", observer);
  vi.stubGlobal("IntersectionObserver", observer);
});

async function dirtyAndRequestClose() {
  render(<Page />);
  await waitFor(() => expect(closeHandler).not.toBeNull());
  await waitFor(() => expect(api.get).toHaveBeenCalled());
  await act(async () => {});
  fireEvent.click(screen.getByRole("button", { name: "settings.jsonMode" }));
  fireEvent.change(screen.getByRole("textbox", { name: "JSON" }), { target: { value: '{"model":"draft"}' } });
  act(() => closeHandler!({ preventDefault: vi.fn() }));
}

describe("关窗 × 未保存的 Settings", () => {
  it("有未保存改动：弹确认且不销毁；放弃更改后销毁", async () => {
    await dirtyAndRequestClose();
    expect(await screen.findByTestId("unsaved-modal")).toBeInTheDocument();
    expect(destroy).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTestId("unsaved-discard"));
    await waitFor(() => expect(destroy).toHaveBeenCalledTimes(1));
  });
});

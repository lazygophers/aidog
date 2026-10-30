import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor } from "../test/render";
import { CodexSettings } from "./CodexSettings";
import { PiSettings } from "./PiSettings";

const codex = vi.hoisted(() => ({ read: vi.fn(), write: vi.fn() }));
const pi = vi.hoisted(() => ({ read: vi.fn(), write: vi.fn() }));
const presetList = vi.hoisted(() => vi.fn());
vi.mock("../services/api", () => ({
  codexApi: codex,
  piApi: pi,
  settingsPresetApi: { list: presetList },
}));
vi.mock("../components/shared/JsonCodeEditor", () => ({
  JsonCodeEditor: ({ value }: { value: string }) => <textarea aria-label="JSON" value={value} readOnly />,
}));

const codexOss = { id: "oss", name: "oss", config: { model: "m", model_providers: { aidog: { base_url: "http://127.0.0.1:{{port}}/proxy" } } } };
const piOss = { id: "oss", name: "oss", config: { theme: "dark" } };

beforeEach(() => {
  vi.clearAllMocks();
  const observer = class { observe() {} disconnect() {} unobserve() {} };
  vi.stubGlobal("ResizeObserver", observer);
  vi.stubGlobal("IntersectionObserver", observer);
});

async function jsonText(el: React.ReactElement) {
  render(el);
  await act(async () => {});
  fireEvent.click(screen.getByRole("button", { name: "settings.jsonMode" }));
  return JSON.parse((screen.getByRole("textbox", { name: "JSON" }) as HTMLTextAreaElement).value);
}

describe("Codex 页预设", () => {
  it("首次无配置：自动填入 oss，{{port}} 渲染为默认端口", async () => {
    presetList.mockResolvedValue([codexOss]);
    codex.read.mockResolvedValue({});
    const v = await jsonText(<CodexSettings />);
    expect(presetList).toHaveBeenCalledWith("codex");
    expect(v.model_providers.aidog.base_url).toBe("http://127.0.0.1:9890/proxy");
  });

  it("1 份：加载推荐配置不弹窗，直接深合并", async () => {
    presetList.mockResolvedValue([codexOss]);
    codex.read.mockResolvedValue({ keep: 1 });
    render(<CodexSettings />);
    await act(async () => {});
    fireEvent.click(screen.getByRole("button", { name: "settings.loadRecommended" }));
    expect(document.querySelector("#preset-picker")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "settings.jsonMode" }));
    const v = JSON.parse((screen.getByRole("textbox", { name: "JSON" }) as HTMLTextAreaElement).value);
    expect(v).toMatchObject({ keep: 1, model: "m" });
  });

  it("2 份：弹选择框，选中后合并", async () => {
    presetList.mockResolvedValue([codexOss, { id: "pro", name: "Pro", config: { model: "pro" } }]);
    codex.read.mockResolvedValue({});
    render(<CodexSettings />);
    await act(async () => {});
    fireEvent.click(screen.getByRole("button", { name: "settings.loadRecommended" }));
    await waitFor(() => expect(document.querySelector("#preset-picker")).not.toBeNull());
    fireEvent.click(document.querySelector("#preset-option-pro")!);
    expect(document.querySelector("#preset-picker")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "settings.jsonMode" }));
    const v = JSON.parse((screen.getByRole("textbox", { name: "JSON" }) as HTMLTextAreaElement).value);
    expect(v.model).toBe("pro");
  });
});

describe("Pi 页预设", () => {
  it("首次无配置：自动填入 oss", async () => {
    presetList.mockResolvedValue([piOss]);
    pi.read.mockResolvedValue({});
    const v = await jsonText(<PiSettings />);
    expect(presetList).toHaveBeenCalledWith("pi");
    expect(v).toEqual({ theme: "dark" });
  });

  it("1 份：加载推荐配置不弹窗，直接深合并", async () => {
    presetList.mockResolvedValue([piOss]);
    pi.read.mockResolvedValue({ theme: "light", keep: 1 });
    render(<PiSettings />);
    await act(async () => {});
    fireEvent.click(screen.getByRole("button", { name: "settings.loadRecommended" }));
    expect(document.querySelector("#preset-picker")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "settings.jsonMode" }));
    const v = JSON.parse((screen.getByRole("textbox", { name: "JSON" }) as HTMLTextAreaElement).value);
    expect(v).toEqual({ theme: "dark", keep: 1 });
  });
});

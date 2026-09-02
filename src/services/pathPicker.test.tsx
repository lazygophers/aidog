// 票 10：浏览器形态「选路径」的回归测试。
//
// 盯的是这条链：`pickPath()` → PathPickerHost 弹窗 → 文本框 + 服务端补全 → 确认/取消
// 各 resolve 出什么。桌面形态那半边（原生对话框）另有一条用例，确认它压根不碰弹窗。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen } from "../test/render";
import userEvent from "@testing-library/user-event";
import { PathPickerHost } from "../components/shared/PathPickerHost";
import { pickPath, registerPathPickerHost } from "./pathPicker";

// transport：默认非 Tauri（浏览器形态）。invoke 只会被 fs_autocomplete 用到。
const isTauri = vi.hoisted(() => vi.fn(() => false));
const invoke = vi.hoisted(() => vi.fn());
vi.mock("./transport", () => ({ isTauri, invoke }));

// 原生对话框：桌面分支用，浏览器分支必须一次都不调。
const tauriOpen = vi.hoisted(() => vi.fn());
const tauriSave = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: tauriOpen, save: tauriSave }));

beforeEach(() => {
  isTauri.mockReturnValue(false);
  invoke.mockReset();
  invoke.mockResolvedValue([]);
  tauriOpen.mockReset();
  tauriSave.mockReset();
});
afterEach(() => registerPathPickerHost(null));

describe("浏览器形态", () => {
  it("确认后 resolve 输入的路径，且全程不碰原生对话框", async () => {
    const user = userEvent.setup();
    render(<PathPickerHost />);

    const promise = pickPath({ directory: true });
    const input = await screen.findByRole("textbox");
    await user.type(input, "/tmp/x");
    await user.click(screen.getByRole("button", { name: "action.confirm" }));

    await expect(promise).resolves.toBe("/tmp/x");
    expect(tauriOpen).not.toHaveBeenCalled();
    expect(tauriSave).not.toHaveBeenCalled();
  });

  it("取消 resolve null（不是抛错，调用方按「用户取消」处理）", async () => {
    const user = userEvent.setup();
    render(<PathPickerHost />);

    const promise = pickPath({});
    await screen.findByRole("textbox");
    await user.click(screen.getByRole("button", { name: "common.cancel" }));

    await expect(promise).resolves.toBeNull();
  });

  it("输入路径时走 fs_autocomplete 拿补全（浏览器里它是原生对话框的替代品）", async () => {
    const user = userEvent.setup();
    invoke.mockResolvedValue([
      { name: "Documents", full_path: "/Users/me/Documents", is_dir: true, modified: 0 },
    ]);
    render(<PathPickerHost />);

    void pickPath({});
    const input = await screen.findByRole("textbox");
    await user.type(input, "~/");

    await vi.waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("fs_autocomplete", expect.objectContaining({ input: "~/" })),
    );
    expect(await screen.findByText("Documents")).toBeInTheDocument();
  });

  it("defaultPath 预填进输入框", async () => {
    render(<PathPickerHost />);
    void pickPath({ save: true, defaultPath: "backup.aidogx" });
    expect(await screen.findByRole("textbox")).toHaveValue("backup.aidogx");
  });

  it("没挂 Host 就调 pickPath 会明确报错，而不是静默当成用户取消", async () => {
    registerPathPickerHost(null);
    await expect(pickPath({})).rejects.toThrow(/host not mounted/);
  });
});

describe("桌面形态", () => {
  it("走原生 open 对话框，不渲染弹窗", async () => {
    isTauri.mockReturnValue(true);
    tauriOpen.mockResolvedValue("/picked/dir");
    render(<PathPickerHost />);

    await expect(pickPath({ directory: true, title: "t" })).resolves.toBe("/picked/dir");
    expect(tauriOpen).toHaveBeenCalledWith(
      expect.objectContaining({ directory: true, multiple: false, title: "t" }),
    );
    expect(screen.queryByRole("textbox")).toBeNull();
  });

  it("save:true 走原生保存对话框；用户取消（null）归一成 null", async () => {
    isTauri.mockReturnValue(true);
    tauriSave.mockResolvedValue(null);
    render(<PathPickerHost />);

    await expect(pickPath({ save: true, defaultPath: "a.aidogx" })).resolves.toBeNull();
    expect(tauriSave).toHaveBeenCalledWith(expect.objectContaining({ defaultPath: "a.aidogx" }));
    expect(tauriOpen).not.toHaveBeenCalled();
  });
});

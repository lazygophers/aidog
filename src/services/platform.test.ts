// 票 10：桌面原生能力兜底的回归测试。
//
// 每条能力都要证明两件事：桌面走 Tauri 插件、浏览器走标准 Web API。
// 走错一边的后果是「换个形态就少个功能」，而这正是本票要根除的。

import { describe, it, expect, vi, beforeEach } from "vitest";

const isTauri = vi.hoisted(() => vi.fn(() => false));
const invoke = vi.hoisted(() => vi.fn());
vi.mock("./transport", () => ({ isTauri, invoke }));

const tauriWriteText = vi.hoisted(() => vi.fn(() => Promise.resolve()));
const tauriReadText = vi.hoisted(() => vi.fn(() => Promise.resolve("from-tauri")));
vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({
  writeText: tauriWriteText,
  readText: tauriReadText,
}));

const tauriOpenUrl = vi.hoisted(() => vi.fn(() => Promise.resolve()));
const tauriReveal = vi.hoisted(() => vi.fn(() => Promise.resolve()));
vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: tauriOpenUrl,
  revealItemInDir: tauriReveal,
}));

const tauriGetVersion = vi.hoisted(() => vi.fn(() => Promise.resolve("9.9.9")));
vi.mock("@tauri-apps/api/app", () => ({ getVersion: tauriGetVersion }));

const isPermissionGranted = vi.hoisted(() => vi.fn(() => Promise.resolve(true)));
const requestPermission = vi.hoisted(() => vi.fn(() => Promise.resolve("granted")));
const tauriSendNotification = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted,
  requestPermission,
  sendNotification: tauriSendNotification,
}));

import {
  writeText, readText, openUrl, revealItemInDir, canRevealItemInDir, getAppVersion, notify,
} from "./platform";

beforeEach(() => {
  isTauri.mockReturnValue(false);
  vi.clearAllMocks();
});

describe("剪贴板", () => {
  it("桌面走 Tauri 插件", async () => {
    isTauri.mockReturnValue(true);
    await writeText("hi");
    expect(tauriWriteText).toHaveBeenCalledWith("hi");
  });

  it("浏览器走 navigator.clipboard", async () => {
    const wt = vi.fn(() => Promise.resolve());
    vi.stubGlobal("navigator", { clipboard: { writeText: wt } });
    await writeText("hi");
    expect(wt).toHaveBeenCalledWith("hi");
    expect(tauriWriteText).not.toHaveBeenCalled();
    vi.unstubAllGlobals();
  });

  it("非安全上下文（无 navigator.clipboard）退回 execCommand，不是直接失败", async () => {
    // 内核绑到局域网时页面是 http://192.168.x.x，浏览器判定非安全上下文 → clipboard 为 undefined。
    vi.stubGlobal("navigator", {});
    const execCommand = vi.fn(() => true);
    (document as unknown as { execCommand: unknown }).execCommand = execCommand;
    await writeText("fallback");
    expect(execCommand).toHaveBeenCalledWith("copy");
    vi.unstubAllGlobals();
  });

  it("读剪贴板：桌面用插件，浏览器用 navigator", async () => {
    isTauri.mockReturnValue(true);
    expect(await readText()).toBe("from-tauri");

    isTauri.mockReturnValue(false);
    vi.stubGlobal("navigator", { clipboard: { readText: () => Promise.resolve("from-web") } });
    expect(await readText()).toBe("from-web");
    vi.unstubAllGlobals();
  });
});

describe("打开链接 / 定位文件", () => {
  it("桌面用系统浏览器，浏览器开新标签页", async () => {
    isTauri.mockReturnValue(true);
    await openUrl("https://x.dev");
    expect(tauriOpenUrl).toHaveBeenCalledWith("https://x.dev");

    isTauri.mockReturnValue(false);
    const open = vi.fn();
    vi.stubGlobal("open", open);
    await openUrl("https://x.dev");
    expect(open).toHaveBeenCalledWith("https://x.dev", "_blank", "noopener,noreferrer");
    vi.unstubAllGlobals();
  });

  it("浏览器里没有「在文件夹显示」，退化成复制路径（UI 据此换文案）", async () => {
    expect(canRevealItemInDir()).toBe(false);
    const wt = vi.fn(() => Promise.resolve());
    vi.stubGlobal("navigator", { clipboard: { writeText: wt } });
    await revealItemInDir("/tmp/backup.aidogx");
    expect(tauriReveal).not.toHaveBeenCalled();
    expect(wt).toHaveBeenCalledWith("/tmp/backup.aidogx");
    vi.unstubAllGlobals();

    isTauri.mockReturnValue(true);
    expect(canRevealItemInDir()).toBe(true);
    await revealItemInDir("/tmp/backup.aidogx");
    expect(tauriReveal).toHaveBeenCalledWith("/tmp/backup.aidogx");
  });
});

describe("应用版本", () => {
  it("浏览器里问后端 about_info 要，不返回空", async () => {
    invoke.mockResolvedValue({ app_version: "1.2.3" });
    expect(await getAppVersion()).toBe("1.2.3");
    expect(invoke).toHaveBeenCalledWith("about_info");
  });

  it("桌面直接用 Tauri 的 getVersion", async () => {
    isTauri.mockReturnValue(true);
    expect(await getAppVersion()).toBe("9.9.9");
    expect(invoke).not.toHaveBeenCalled();
  });
});

describe("系统通知", () => {
  it("桌面走插件（先要权限再发）", async () => {
    isTauri.mockReturnValue(true);
    await notify("t", "b");
    expect(tauriSendNotification).toHaveBeenCalledWith({ title: "t", body: "b" });
  });

  it("浏览器走 window.Notification", async () => {
    const ctor = vi.fn();
    class FakeNotification {
      static permission = "granted";
      static requestPermission = vi.fn();
      constructor(title: string, opts: { body: string }) { ctor(title, opts); }
    }
    vi.stubGlobal("Notification", FakeNotification);
    await notify("t", "b");
    expect(ctor).toHaveBeenCalledWith("t", { body: "b" });
    vi.unstubAllGlobals();
  });

  it("通知不可用时静默跳过，不把调用方拖崩", async () => {
    vi.stubGlobal("Notification", undefined);
    await expect(notify("t", "b")).resolves.toBeUndefined();
    vi.unstubAllGlobals();
  });
});

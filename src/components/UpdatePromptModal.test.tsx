// UpdatePromptModal 就近测试：字节进度、验签完成后的重启确认、安装阶段与错误展示。
// download/install mocks 手动驱动状态，不连接真实更新服务。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, fireEvent } from "@testing-library/react";
import { render, screen } from "../test/render";
import { UpdatePromptModal } from "./UpdatePromptModal";

const downloadMock = vi.fn();
const installMock = vi.fn();
const beginDrainMock = vi.fn();
const abortDrainMock = vi.fn();
const relaunchWhenDrainedMock = vi.fn();

vi.mock("../services/updater", () => ({
  downloadUpdate: (update: unknown, onProgress?: (...a: unknown[]) => void) => downloadMock(onProgress),
  installUpdate: (update: unknown) => installMock(update),
  beginDrain: () => beginDrainMock(),
  abortDrain: () => abortDrainMock(),
  relaunchWhenDrained: () => relaunchWhenDrainedMock(),
}));

vi.mock("@tauri-apps/plugin-updater", () => ({}));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: vi.fn() }));

const update = { version: "9.9.9", body: "notes" } as never;
const onClose = vi.fn();

beforeEach(() => {
  downloadMock.mockReset();
  installMock.mockReset();
  beginDrainMock.mockReset();
  abortDrainMock.mockReset();
  // abortDrain 返回 promise（解除排空的调用方都 .catch 兜底）。
  abortDrainMock.mockResolvedValue(true);
  relaunchWhenDrainedMock.mockReset();
  onClose.mockReset();
});

describe("UpdatePromptModal — 更新进度", () => {
  it("显示未知大小的已下载字节，不伪造百分比", () => {
    let cb: ((...a: unknown[]) => void) | undefined;
    downloadMock.mockImplementation((onProgress?: (...a: unknown[]) => void) => {
      cb = onProgress;
      return new Promise(() => {});
    });
    render(<UpdatePromptModal update={update} onClose={onClose} />);
    fireEvent.click(screen.getByTestId("update-start"));
    act(() => { cb?.(1024, undefined); });
    const el = screen.getByTestId("update-progress");
    expect(el.dataset.phase).toBe("downloading");
    expect(el.getAttribute("data-pct")).toBeNull();
    expect(el.textContent).toContain("updater.downloadedUnknown");
  });

  it("已知总大小：显示百分比且不超过 100", () => {
    let cb: ((...a: unknown[]) => void) | undefined;
    downloadMock.mockImplementation((onProgress?: (...a: unknown[]) => void) => {
      cb = onProgress;
      return new Promise(() => {});
    });
    render(<UpdatePromptModal update={update} onClose={onClose} />);
    fireEvent.click(screen.getByTestId("update-start"));
    act(() => { cb?.(1500, 1000); });
    const el = screen.getByTestId("update-progress");
    expect(el.dataset.phase).toBe("downloading");
    expect(el.getAttribute("data-pct")).toBe("100");
    expect(el.textContent).toContain("updater.downloadProgress");
  });

  it("确认后先排空（拒新请求）再安装，等待在途清零后由内核侧重启", async () => {
    downloadMock.mockImplementation(async (onProgress?: (...a: unknown[]) => void) => {
      onProgress?.(1500, 1000);
    });
    const order: string[] = [];
    beginDrainMock.mockImplementation(async () => { order.push("drain"); return true; });
    installMock.mockImplementation(async () => { order.push("install"); });
    relaunchWhenDrainedMock.mockImplementation(async () => { order.push("relaunch"); });
    render(<UpdatePromptModal update={update} onClose={onClose} />);
    fireEvent.click(screen.getByTestId("update-start"));
    const restart = await screen.findByTestId("update-restart");
    expect(screen.getByTestId("update-progress").dataset.phase).toBe("ready");
    expect(screen.getByTestId("update-progress").textContent).toContain("updater.readyToRestart");
    expect(beginDrainMock).not.toHaveBeenCalled();
    fireEvent.click(restart);
    await screen.findByText("updater.draining");
    expect(screen.getByTestId("update-progress").dataset.phase).toBe("draining");
    expect(order).toEqual(["drain", "install", "relaunch"]);
    expect(relaunchWhenDrainedMock).toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });

  it("下载失败：显示原因，不安装，稍后可关闭", async () => {
    downloadMock.mockRejectedValue(new Error("download failed"));
    render(<UpdatePromptModal update={update} onClose={onClose} />);
    fireEvent.click(screen.getByTestId("update-start"));
    const err = await screen.findByTestId("update-error");
    expect(err.textContent).toContain("updater.updateFailed");
    expect(err.textContent).toContain("download failed");
    expect(installMock).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTestId("update-later"));
    expect(onClose).toHaveBeenCalled();
  });

  it("安装或重启失败：显示原因", async () => {
    downloadMock.mockResolvedValue(undefined);
    installMock.mockRejectedValue(new Error("install failed"));
    render(<UpdatePromptModal update={update} onClose={onClose} />);
    fireEvent.click(screen.getByTestId("update-start"));
    fireEvent.click(await screen.findByTestId("update-restart"));
    const err = await screen.findByTestId("update-error");
    expect(err.textContent).toContain("updater.updateFailed");
    expect(err.textContent).toContain("install failed");
    expect(onClose).not.toHaveBeenCalled();
  });

  it("安装失败：解除排空并回到可重试，稍后关闭也解除排空", async () => {
    downloadMock.mockResolvedValue(undefined);
    installMock.mockRejectedValue(new Error("install failed"));
    beginDrainMock.mockResolvedValue(true);
    abortDrainMock.mockResolvedValue(true);
    render(<UpdatePromptModal update={update} onClose={onClose} />);
    fireEvent.click(screen.getByTestId("update-start"));
    fireEvent.click(await screen.findByTestId("update-restart"));
    await screen.findByTestId("update-error");
    // 排空已置位：失败即解除，否则桌面代理所有请求恒 503
    expect(abortDrainMock).toHaveBeenCalled();
    // 界面回到 ready，可再次点「重启安装」重试
    expect(screen.getByTestId("update-progress").dataset.phase).toBe("ready");
    const retry = screen.getByTestId("update-restart");
    expect(retry).not.toBeDisabled();
    // 稍后：同样解除排空后关闭
    fireEvent.click(screen.getByTestId("update-later"));
    expect(abortDrainMock).toHaveBeenCalledTimes(2);
    expect(onClose).toHaveBeenCalled();
  });
});

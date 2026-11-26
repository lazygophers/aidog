// updater service 的拆分契约：download() 回调进度并先完成验签；install() 仅装包不重启；
// 排空与重启走内核命令 update_begin_drain / update_relaunch_when_drained。
import { describe, it, expect, vi } from "vitest";
import type { Update } from "@tauri-apps/plugin-updater";
import { abortDrain, beginDrain, downloadUpdate, installUpdate, relaunchWhenDrained } from "./updater";

const invokeMock = vi.fn();
vi.mock("@tauri-apps/plugin-updater", () => ({ check: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invokeMock(...a) }));

describe("updater service", () => {
  it("等待下载与验签完成后，安装不重启（重启由排空链路接管）", async () => {
    const order: string[] = [];
    const update = {
      download: vi.fn(async (onEvent: (event: never) => void) => {
        order.push("download");
        onEvent({ event: "Started", data: { contentLength: 10 } } as never);
        onEvent({ event: "Progress", data: { chunkLength: 3 } } as never);
        onEvent({ event: "Progress", data: { chunkLength: 4 } } as never);
        onEvent({ event: "Finished" } as never);
        // 真实 Update.download 在返回前完成签名校验。
      }),
      install: vi.fn(async () => { order.push("install"); }),
    } as unknown as Update;
    const progress = vi.fn();

    await downloadUpdate(update, progress);
    expect(order).toEqual(["download"]);
    expect(update.install).not.toHaveBeenCalled();
    expect(progress.mock.calls).toEqual([[0, 10], [3, 10], [7, 10], [7, 10]]);

    await installUpdate(update);
    expect(order).toEqual(["download", "install"]);
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("beginDrain / relaunchWhenDrained 走内核排空命令", async () => {
    invokeMock.mockResolvedValue(true);
    await expect(beginDrain()).resolves.toBe(true);
    expect(invokeMock).toHaveBeenLastCalledWith("update_begin_drain");
    invokeMock.mockResolvedValue(undefined);
    await relaunchWhenDrained();
    expect(invokeMock).toHaveBeenLastCalledWith("update_relaunch_when_drained");
    invokeMock.mockResolvedValue(false);
    await expect(abortDrain()).resolves.toBe(false);
    expect(invokeMock).toHaveBeenLastCalledWith("update_abort_drain");
  });

  it("下载或验签失败时不调用安装与排空", async () => {
    const update = {
      download: vi.fn().mockRejectedValue(new Error("signature invalid")),
      install: vi.fn(),
    } as unknown as Update;

    await expect(downloadUpdate(update)).rejects.toThrow("signature invalid");
    expect(update.install).not.toHaveBeenCalled();
    expect(invokeMock).not.toHaveBeenCalled();
  });
});

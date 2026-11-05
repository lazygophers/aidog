import { describe, it, expect, vi, beforeEach } from "vitest";
import { listenKernelUpdate } from "./updateEvent";

let handler: (() => Promise<void> | void) | null = null;
let eventName = "";
const unlisten = vi.fn();
vi.mock("./transport", () => ({
  listen: async (name: string, h: () => void) => {
    eventName = name;
    handler = h;
    return unlisten;
  },
}));
const emit = vi.fn();
vi.mock("@tauri-apps/api/event", () => ({ emit: (n: string) => emit(n) }));
const check = vi.fn();
vi.mock("@tauri-apps/plugin-updater", () => ({ check: () => check() }));

beforeEach(() => {
  handler = null;
  eventName = "";
  check.mockReset();
  emit.mockReset();
  unlisten.mockClear();
});

describe("listenKernelUpdate", () => {
  it("内核事件到达 → 取 Update 交给回调", async () => {
    const upd = { version: "9.9.9" };
    check.mockResolvedValue(upd);
    const cb = vi.fn();
    await listenKernelUpdate(cb);
    expect(eventName).toBe("aidog-update-available");
    await handler!();
    expect(cb).toHaveBeenCalledWith(upd);
  });

  it("事件到达即 emit ack（内核据此清除挂起版本）", async () => {
    check.mockResolvedValue(null);
    await listenKernelUpdate(vi.fn());
    await handler!();
    expect(emit).toHaveBeenCalledWith("aidog-update-ack");
  });

  it("check 无更新或抛错 → 不回调、不抛", async () => {
    const cb = vi.fn();
    await listenKernelUpdate(cb);
    check.mockResolvedValueOnce(null);
    await handler!();
    check.mockRejectedValueOnce(new Error("offline"));
    await handler!();
    expect(cb).not.toHaveBeenCalled();
  });

  it("返回的取消函数即 listen 的 unlisten", async () => {
    const off = await listenKernelUpdate(vi.fn());
    off();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });
});

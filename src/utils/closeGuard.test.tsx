import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, act } from "@testing-library/react";
import { useCloseGuard } from "./closeGuard";
import { registerNavGuard } from "./navGuard";

// 假窗口：保留 onCloseRequested 回调，测试里手动触发「用户点关闭」。
let closeHandler: ((e: { preventDefault: () => void }) => void) | null = null;
const destroy = vi.fn();
const unlisten = vi.fn();
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    onCloseRequested: async (h: typeof closeHandler) => {
      closeHandler = h;
      return unlisten;
    },
    destroy,
  }),
}));

function Harness() {
  useCloseGuard();
  return <div data-testid="harness" />;
}

async function mount() {
  const r = render(<Harness />);
  await waitFor(() => expect(closeHandler).not.toBeNull());
  return r;
}

function requestClose() {
  const preventDefault = vi.fn();
  act(() => closeHandler!({ preventDefault }));
  return preventDefault;
}

beforeEach(() => {
  closeHandler = null;
  destroy.mockClear();
  unlisten.mockClear();
});

describe("useCloseGuard", () => {
  it("无未保存改动：关窗请求直接销毁", async () => {
    await mount();
    requestClose();
    expect(destroy).toHaveBeenCalledTimes(1);
  });

  it("有 guard：拦下关窗，确认前不销毁；确认后销毁", async () => {
    let proceed: (() => void) | undefined;
    const off = registerNavGuard((p) => { proceed = p; });
    await mount();
    const prevented = requestClose();
    expect(prevented).toHaveBeenCalled();
    expect(destroy).not.toHaveBeenCalled();
    act(() => proceed!());
    expect(destroy).toHaveBeenCalledTimes(1);
    off();
  });

  it("有 guard：取消（不调 proceed）则窗口保留", async () => {
    const off = registerNavGuard(() => {});
    await mount();
    requestClose();
    expect(destroy).not.toHaveBeenCalled();
    off();
  });

  it("卸载时取消监听", async () => {
    const r = await mount();
    r.unmount();
    await waitFor(() => expect(unlisten).toHaveBeenCalled());
    expect(screen.queryByTestId("harness")).toBeNull();
  });
});

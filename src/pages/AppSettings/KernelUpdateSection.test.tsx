// 票07：内核更新区块测试。mock services/api（即 mock /api/rpc 传输层之上的服务封装），
// 断言只依赖内核管理接口的返回形状（镜像 aidog_kernel::update::status() 的 serde 输出）。
// 测试 i18n 缺 key 回 key 本身（render.tsx 口径），文案断言用 key；版本值走 data-version。
import { beforeEach, afterEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "../../test/render";
import { KernelUpdateSection } from "./KernelUpdateSection";

const h = vi.hoisted(() => ({
  status: vi.fn(),
  check: vi.fn(),
  download: vi.fn(),
  install: vi.fn(),
  confirm: vi.fn(),
}));
vi.mock("../../services/api", () => ({
  kernelUpdateApi: {
    status: h.status,
    check: h.check,
    download: h.download,
    install: h.install,
    confirm: h.confirm,
  },
}));

const idle = (version = "1.2.2") => ({
  current_version: version,
  platform_key: "darwin-aarch64-kernel",
  updates_enabled: true,
  inflight: 0,
  phase: { phase: "idle" },
});

const setup = () => {
  h.status.mockResolvedValue(idle());
  h.check.mockResolvedValue({ available: false });
  h.download.mockResolvedValue({ started: true });
  h.install.mockResolvedValue({ installed: true, version: "1.3.0" });
  h.confirm.mockResolvedValue({ restarting: true });
};

// fake timers 下 waitFor 不走时钟：统一用 act 推进一个轮询周期 + 冲掉微任务
const tick = () => act(async () => { await vi.advanceTimersByTimeAsync(1000); });
const flush = () => act(async () => {});
const click = (id: string) => { fireEvent.click(screen.getByTestId(id)); return flush(); };

describe("KernelUpdateSection", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    setup();
  });
  afterEach(() => { vi.useRealTimers(); });

  it("shows current version from kernel_update_status", async () => {
    render(<KernelUpdateSection />);
    await tick();
    expect(screen.getByTestId("kernel-update-version")).toHaveTextContent("kernelUpdate.currentVersion");
    expect(screen.getByTestId("kernel-update-version")).toHaveAttribute("data-version", "1.2.2");
    expect(screen.getByTestId("kernel-update-section")).toHaveAttribute("data-phase", "idle");
  });

  it("unreachable: never got status → shows unavailable, no version row", async () => {
    h.status.mockRejectedValue("unauthorized");
    render(<KernelUpdateSection />);
    await tick();
    expect(screen.getByTestId("kernel-update-unreachable")).toBeInTheDocument();
    expect(screen.queryByTestId("kernel-update-version")).toBeNull();
  });

  it("check → new version available; in-flight download locks the button (mutex)", async () => {
    h.check.mockResolvedValue({ available: true, version: "1.3.0" });
    let release!: () => void;
    h.download.mockImplementation(() => new Promise<void>((r) => { release = r; }));
    render(<KernelUpdateSection />);
    await tick();
    await click("kernel-update-check");
    expect(screen.getByTestId("kernel-update-available")).toHaveAttribute("data-version", "1.3.0");
    await click("kernel-update-download");
    expect(screen.getByTestId("kernel-update-download")).toBeDisabled(); // RPC 在途 → 互斥
    fireEvent.click(screen.getByTestId("kernel-update-download")); // disabled 按钮不吃点击
    release();
    await flush();
    expect(h.download).toHaveBeenCalledTimes(1);
    expect(h.check).toHaveBeenCalledTimes(1);
  });

  it("downloading shows progress; install unlocks restart; confirm dialog triggers confirm RPC", async () => {
    h.check.mockResolvedValue({ available: true, version: "1.3.0" });
    render(<KernelUpdateSection />);
    await tick();
    await click("kernel-update-check");
    await click("kernel-update-download");
    expect(h.download).toHaveBeenCalledTimes(1);

    h.status.mockResolvedValue({ ...idle(), phase: { phase: "downloading", downloaded: 1536, total: 4096 } });
    await tick();
    expect(screen.getByTestId("kernel-update-section")).toHaveAttribute("data-phase", "downloading");
    expect(screen.getByTestId("kernel-update-progress")).toBeInTheDocument();

    h.status.mockResolvedValue({ ...idle(), phase: { phase: "ready_to_install", version: "1.3.0", staged: "/x/pkg" } });
    await tick();
    expect(screen.getByTestId("kernel-update-section")).toHaveAttribute("data-phase", "ready_to_install");
    // 未 install 前无重启按钮
    expect(screen.queryByTestId("kernel-update-restart")).toBeNull();
    await click("kernel-update-install");
    expect(h.install).toHaveBeenCalledTimes(1);
    await tick();
    await click("kernel-update-restart");
    expect(screen.getByTestId("kernel-update-confirm")).toBeInTheDocument();
    await click("kernel-update-confirm");
    expect(h.confirm).toHaveBeenCalledTimes(1);

    h.status.mockResolvedValue({ ...idle(), inflight: 3, phase: { phase: "restarting" } });
    await tick();
    expect(screen.getByTestId("kernel-update-section")).toHaveAttribute("data-phase", "restarting");
    expect(screen.getByTestId("kernel-update-restarting")).toHaveTextContent("kernelUpdate.restarting");
  });

  it("failed phase shows the kernel-side failure reason; failed still allows check retry", async () => {
    h.status.mockResolvedValue({ ...idle(), phase: { phase: "failed", error: "signature verification failed" } });
    render(<KernelUpdateSection />);
    await tick();
    expect(screen.getByTestId("kernel-update-failed")).toHaveTextContent("signature verification failed");
    expect(screen.getByTestId("kernel-update-check")).not.toBeDisabled();
  });
});

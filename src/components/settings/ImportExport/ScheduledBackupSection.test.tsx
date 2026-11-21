// 定时备份状态的事件驱动刷新：backup-completed → 重拉设置；隐藏期间跳过，恢复可见补拉。
import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor } from "../../../test/render";

const api = vi.hoisted(() => ({ get: vi.fn(), set: vi.fn(), runNow: vi.fn(), listen: vi.fn() }));
vi.mock("../../../services/api", () => ({ backupApi: api }));
vi.mock("../../../services/platform", () => ({ revealItemInDir: vi.fn(), canRevealItemInDir: () => false }));
vi.mock("../../../services/transport", () => ({ listen: api.listen }));

let onBackupCompleted: (() => void) | undefined;

import { ScheduledBackupSection } from "./ScheduledBackupSection";

const SETTINGS = { enabled: true, interval_hours: 6, retention_days: 7, last_backup_at: 1700000000000, last_backup_error: "" };

function setHidden(v: boolean) {
  Object.defineProperty(document, "hidden", { value: v, configurable: true });
  Object.defineProperty(document, "visibilityState", { value: v ? "hidden" : "visible", configurable: true });
}

beforeEach(() => {
  vi.clearAllMocks();
  api.get.mockResolvedValue(SETTINGS);
  api.set.mockResolvedValue(SETTINGS);
  api.runNow.mockResolvedValue({ ok: true, path: "/tmp/x.aidogx" });
  onBackupCompleted = undefined;
  api.listen.mockImplementation((_event: string, handler: () => void) => {
    onBackupCompleted = handler;
    return Promise.resolve(() => {});
  });
  setHidden(false);
});

describe("ScheduledBackupSection", () => {
  it("backup-completed 后重拉设置", async () => {
    render(<ScheduledBackupSection />);
    await screen.findByText(/settings.backup.lastBackup/);
    await waitFor(() => expect(onBackupCompleted).toBeDefined());
    await act(async () => onBackupCompleted?.());
    await waitFor(() => expect(api.get).toHaveBeenCalledTimes(2));
  });

  it("隐藏期间事件不重拉，恢复可见补拉一次", async () => {
    render(<ScheduledBackupSection />);
    await screen.findByText(/settings.backup.lastBackup/);
    await waitFor(() => expect(onBackupCompleted).toBeDefined());
    setHidden(true);
    await act(async () => onBackupCompleted?.());
    expect(api.get).toHaveBeenCalledTimes(1);
    setHidden(false);
    await act(async () => { document.dispatchEvent(new Event("visibilitychange")); });
    await waitFor(() => expect(api.get).toHaveBeenCalledTimes(2));
    setHidden(false);
  });

  it("立即备份成功后刷新 last_backup_at", async () => {
    render(<ScheduledBackupSection />);
    fireEvent.click(await screen.findByText(/settings.backup.runNow/));
    await waitFor(() => expect(api.runNow).toHaveBeenCalled());
    await waitFor(() => expect(api.get).toHaveBeenCalledTimes(2));
  });
});

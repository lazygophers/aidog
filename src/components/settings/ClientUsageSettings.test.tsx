// 设置子页「客户端会话读取」就近测试：渲染真组件，mock api，断言走 data-testid / data-state。
import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "../../test/render";

const api = vi.hoisted(() => ({
  consentGet: vi.fn(),
  consentSet: vi.fn(),
  retentionGet: vi.fn(),
  retentionSet: vi.fn(),
  transcriptConsentGet: vi.fn(),
  transcriptConsentSet: vi.fn(),
}));
vi.mock("../../services/api/clientUsage", () => ({ clientUsageApi: api }));

import { ClientUsageSettings } from "./ClientUsageSettings";

const CODES = ["claude_code", "codex", "gemini_cli", "opencode"];

beforeEach(() => {
  vi.clearAllMocks();
  api.consentGet.mockResolvedValue({ asked: false, enabled: {} });
  api.consentSet.mockResolvedValue(undefined);
  api.retentionGet.mockResolvedValue(90);
  api.retentionSet.mockResolvedValue(undefined);
  api.transcriptConsentGet.mockResolvedValue(false);
  api.transcriptConsentSet.mockResolvedValue(undefined);
});

describe("ClientUsageSettings", () => {
  it("渲染四个读取开关（默认关）、保留天数 90、原文开关默认关", async () => {
    render(<ClientUsageSettings />);
    await waitFor(() => expect((screen.getByTestId("retention-days") as HTMLInputElement).value).toBe("90"));
    for (const code of CODES) {
      expect(screen.getByTestId(`client-switch-${code}`).getAttribute("data-state")).toBe("unchecked");
    }
    expect(screen.getByTestId("transcript-consent").getAttribute("data-state")).toBe("unchecked");
    expect(api.consentSet).not.toHaveBeenCalled();
  });

  it("切换客户端开关即写 consent_set 且 asked=true", async () => {
    api.consentGet.mockResolvedValue({ asked: true, enabled: { claude_code: true } });
    render(<ClientUsageSettings />);
    await waitFor(() =>
      expect(screen.getByTestId("client-switch-claude_code").getAttribute("data-state")).toBe("checked"),
    );
    fireEvent.click(screen.getByTestId("client-switch-codex"));
    await waitFor(() =>
      expect(api.consentSet).toHaveBeenCalledWith({ asked: true, enabled: { claude_code: true, codex: true } }),
    );
    expect(screen.getByTestId("client-switch-codex").getAttribute("data-state")).toBe("checked");
    fireEvent.click(screen.getByTestId("client-switch-claude_code"));
    await waitFor(() =>
      expect(api.consentSet).toHaveBeenLastCalledWith({ asked: true, enabled: { claude_code: false, codex: true } }),
    );
  });

  it("保存保留天数：0 = 永久照常写入；负数不写", async () => {
    render(<ClientUsageSettings />);
    const input = await screen.findByTestId("retention-days");
    await waitFor(() => expect((input as HTMLInputElement).value).toBe("90"));
    fireEvent.change(input, { target: { value: "-1" } });
    fireEvent.click(screen.getByTestId("retention-save"));
    expect(api.retentionSet).not.toHaveBeenCalled();
    fireEvent.change(input, { target: { value: "0" } });
    fireEvent.click(screen.getByTestId("retention-save"));
    await waitFor(() => expect(api.retentionSet).toHaveBeenCalledWith(0));
    await waitFor(() => expect(screen.getByTestId("retention-saved")).toBeTruthy());
  });

  it("切换原文开关写 transcript_consent_set，不碰读取开关", async () => {
    render(<ClientUsageSettings />);
    await waitFor(() => expect(api.transcriptConsentGet).toHaveBeenCalled());
    fireEvent.click(screen.getByTestId("transcript-consent"));
    await waitFor(() => expect(api.transcriptConsentSet).toHaveBeenCalledWith(true));
    expect(screen.getByTestId("transcript-consent").getAttribute("data-state")).toBe("checked");
    expect(api.consentSet).not.toHaveBeenCalled();
  });

  it("写入失败时回滚开关并显示错误", async () => {
    api.consentSet.mockRejectedValue(new Error("boom"));
    render(<ClientUsageSettings />);
    await waitFor(() => expect(api.consentGet).toHaveBeenCalled());
    fireEvent.click(screen.getByTestId("client-switch-codex"));
    await waitFor(() => expect(screen.getByTestId("client-usage-settings-error").textContent).toContain("boom"));
    expect(screen.getByTestId("client-switch-codex").getAttribute("data-state")).toBe("unchecked");
  });
});

import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "../../test/render";

const h = vi.hoisted(() => ({ get: vi.fn(), set: vi.fn(), preview: vi.fn() }));
vi.mock("../../services/api/privacy", () => ({ privacyApi: h }));

import { PrivacyScrubSection } from "./PrivacyScrubSection";

const initial = {
  enabled: false,
  secrets: true,
  personal: false,
  model_enabled: false,
  model_endpoint: "",
  per_kind: { EMAIL: true },
};

beforeEach(() => {
  vi.clearAllMocks();
  h.get.mockResolvedValue(structuredClone(initial));
  h.set.mockResolvedValue(undefined);
  h.preview.mockResolvedValue({ original: "a@b.com", scrubbed: "{{PII_EMAIL_1}}", hits: 1 });
});

describe("PrivacyScrubSection", () => {
  it("加载设置：开关状态反映 get 返回值（含 per-kind 覆盖）", async () => {
    render(<PrivacyScrubSection />);
    await screen.findByTestId("privacy-enabled");
    expect(screen.getByTestId("privacy-enabled").getAttribute("data-state")).toBe("unchecked");
    expect(screen.getByTestId("privacy-secrets").getAttribute("data-state")).toBe("checked");
    expect(screen.getByTestId("privacy-personal").getAttribute("data-state")).toBe("unchecked");
    // EMAIL 显式 true 覆盖 personal=false → 行 on；BANK_CARD 无覆盖回落 personal=false → off
    expect(screen.getByTestId("privacy-kind-EMAIL").getAttribute("data-state")).toBe("on");
    expect(screen.getByTestId("privacy-kind-BANK_CARD").getAttribute("data-state")).toBe("off");
  });

  it("读写往返：开总开关保存，set 收完整设置（保留 per_kind）", async () => {
    render(<PrivacyScrubSection />);
    await screen.findByTestId("privacy-enabled");
    fireEvent.click(screen.getByTestId("privacy-enabled"));
    fireEvent.click(screen.getByTestId("privacy-save"));
    await waitFor(() =>
      expect(h.set).toHaveBeenCalledWith(
        expect.objectContaining({ enabled: true, secrets: true, personal: false, per_kind: { EMAIL: true } }),
      ),
    );
    await screen.findByTestId("privacy-saved");
  });

  it("per-kind 覆盖两档总开关：关 EMAIL 行写入显式 false", async () => {
    render(<PrivacyScrubSection />);
    await screen.findByTestId("privacy-enabled");
    fireEvent.click(screen.getByTestId("privacy-kind-switch-EMAIL"));
    fireEvent.click(screen.getByTestId("privacy-save"));
    await waitFor(() =>
      expect(h.set).toHaveBeenCalledWith(expect.objectContaining({ per_kind: { EMAIL: false } })),
    );
  });

  it("无覆盖时点行写入显式值：BANK_CARD off→on", async () => {
    render(<PrivacyScrubSection />);
    await screen.findByTestId("privacy-enabled");
    fireEvent.click(screen.getByTestId("privacy-kind-switch-BANK_CARD"));
    fireEvent.click(screen.getByTestId("privacy-save"));
    await waitFor(() =>
      expect(h.set).toHaveBeenCalledWith(
        expect.objectContaining({ per_kind: { EMAIL: true, BANK_CARD: true } }),
      ),
    );
  });

  it("model_enabled 关时端点输入禁用；开 + 填 URL 进 set 载荷", async () => {
    render(<PrivacyScrubSection />);
    await screen.findByTestId("privacy-enabled");
    expect(screen.getByTestId("privacy-model-endpoint").hasAttribute("disabled")).toBe(true);
    fireEvent.click(screen.getByTestId("privacy-model-enabled"));
    const input = screen.getByTestId("privacy-model-endpoint");
    expect(input.hasAttribute("disabled")).toBe(false);
    fireEvent.change(input, { target: { value: "http://127.0.0.1:9000/detect" } });
    fireEvent.click(screen.getByTestId("privacy-save"));
    await waitFor(() =>
      expect(h.set).toHaveBeenCalledWith(
        expect.objectContaining({ model_enabled: true, model_endpoint: "http://127.0.0.1:9000/detect" }),
      ),
    );
  });

  it("试一试：调 preview 并渲染原文/脱敏后对照与命中数", async () => {
    render(<PrivacyScrubSection />);
    await screen.findByTestId("privacy-enabled");
    fireEvent.change(screen.getByTestId("privacy-preview-input"), { target: { value: "a@b.com" } });
    fireEvent.click(screen.getByTestId("privacy-preview-run"));
    await waitFor(() => expect(h.preview).toHaveBeenCalledWith("a@b.com"));
    expect(await screen.findByTestId("privacy-preview-original")).toHaveTextContent("a@b.com");
    expect(screen.getByTestId("privacy-preview-scrubbed")).toHaveTextContent("{{PII_EMAIL_1}}");
    expect(screen.getByTestId("privacy-preview-hits").getAttribute("data-count")).toBe("1");
  });
});

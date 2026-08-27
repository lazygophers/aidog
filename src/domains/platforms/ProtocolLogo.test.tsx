// ProtocolLogo：缓存命中渲染 <img>，onError（下载坏图/格式坏）回落品牌色首字母圆圈。
// 断言外部可观察行为（渲染了 img 还是文本），不断言 className / 快照。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "../../test/render";
import { ProtocolLogo } from "./ProtocolLogo";
import type { Protocol } from "../../services/api";
import { useProtocolLogo } from "./useProtocolLogo";

vi.mock("./useProtocolLogo", () => ({
  useProtocolLogo: vi.fn(),
}));

// 品牌色来自 registry（getProtocolColorMap 派生自 platform.json 的 color）
vi.mock("./defaults", () => ({
  getProtocolColorMap: vi.fn().mockResolvedValue({ glm_coding: "#3B5FEC" }),
}));

const mockedLogo = vi.mocked(useProtocolLogo);

describe("ProtocolLogo", () => {
  beforeEach(() => {
    mockedLogo.mockReset();
  });

  it("缓存命中 → 渲染 <img>（alt = 协议 code）", () => {
    mockedLogo.mockReturnValue({ logoSrc: "asset://logos/glm_coding.png", fallbackInitial: "G" });
    render(<ProtocolLogo protocol={"glm_coding" as Protocol} />);
    expect(screen.getByAltText("glm_coding")).toBeInTheDocument();
    expect(screen.queryByText("G")).not.toBeInTheDocument();
  });

  it("onError 后渲染首字母圆圈而非 <img>（不出破图）", async () => {
    mockedLogo.mockReturnValue({ logoSrc: "asset://logos/glm_coding.png", fallbackInitial: "G" });
    render(<ProtocolLogo protocol={"glm_coding" as Protocol} />);
    fireEvent.error(screen.getByAltText("glm_coding"));
    await waitFor(() => expect(screen.getByText("G")).toBeInTheDocument());
    expect(screen.queryByAltText("glm_coding")).not.toBeInTheDocument();
  });

  it("缓存 miss（logoSrc 空）→ 直接首字母圆圈，不渲染 <img>", () => {
    mockedLogo.mockReturnValue({ logoSrc: null, fallbackInitial: "D" });
    render(<ProtocolLogo protocol={"deepseek" as Protocol} />);
    expect(screen.getByText("D")).toBeInTheDocument();
    expect(screen.queryByAltText("deepseek")).not.toBeInTheDocument();
  });
});

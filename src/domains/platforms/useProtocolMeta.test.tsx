// 票 11 共享 hook：useProtocolLabelMap / useProtocolColorMap / useProtocolLabel /
// useClientTypeOptions。断言首渲染初始值（{}/""/[]）、异步解析、lang 变化重拉、
// 失败 fallback（clientTypeOptions catch 保空列表）。不断言内部 effect 结构。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";
import type { Protocol } from "../../services/api";
import {
  useProtocolLabelMap,
  useProtocolColorMap,
  useProtocolLabel,
  useClientTypeOptions,
} from "./useProtocolMeta";
import * as defaults from "./defaults";

vi.mock("./defaults", () => ({
  getProtocolLabelMap: vi.fn(),
  getProtocolColorMap: vi.fn(),
  getProtocolLabel: vi.fn(),
  buildClientTypesFromPresets: vi.fn(),
}));

const mockedLabelMap = vi.mocked(defaults.getProtocolLabelMap);
const mockedColorMap = vi.mocked(defaults.getProtocolColorMap);
const mockedLabel = vi.mocked(defaults.getProtocolLabel);
const mockedClientTypes = vi.mocked(defaults.buildClientTypesFromPresets);

describe("useProtocolLabelMap（票 11 labelMap-only 维度）", () => {
  beforeEach(() => vi.clearAllMocks());

  it("首渲染 {}，解析后返回全表；lang 变化重拉", async () => {
    mockedLabelMap
      .mockResolvedValueOnce({ openai: "OpenAI" } as Record<Protocol, string>)
      .mockResolvedValueOnce({ openai: "オープンAI" } as Record<Protocol, string>);
    const { result, rerender } = renderHook(({ lang }) => useProtocolLabelMap(lang), {
      initialProps: { lang: "en-US" },
    });
    expect(result.current).toEqual({});
    await waitFor(() => expect(result.current).toEqual({ openai: "OpenAI" }));
    rerender({ lang: "ja-JP" });
    await waitFor(() => expect(result.current).toEqual({ openai: "オープンAI" }));
    expect(mockedLabelMap).toHaveBeenNthCalledWith(2, "ja-JP");
  });

  it("卸载后 cancelled：迟到解析不触发 setState", async () => {
    let resolve!: (m: Record<string, string>) => void;
    mockedLabelMap.mockImplementationOnce(() => new Promise(r => { resolve = r; }));
    const { unmount } = renderHook(() => useProtocolLabelMap("en-US"));
    unmount();
    await act(async () => { resolve({ openai: "late" }); });
    // 无「update on unmounted」告警即通过
  });
});

describe("useProtocolColorMap（票 11 colorMap-only 维度）", () => {
  beforeEach(() => vi.clearAllMocks());

  it("首渲染 {}，解析后返回 map；无 locale 维度", async () => {
    mockedColorMap.mockResolvedValueOnce({ deepseek: "#1E90FF" });
    const { result } = renderHook(() => useProtocolColorMap());
    expect(result.current).toEqual({});
    await waitFor(() => expect(result.current).toEqual({ deepseek: "#1E90FF" }));
    expect(mockedColorMap).toHaveBeenCalledWith();
  });
});

describe("useProtocolLabel（票 11 单协议 label 维度）", () => {
  beforeEach(() => vi.clearAllMocks());

  it("protocol 缺失 → 恒空串（无 RPC）；有值 → 首渲染 \"\" 后解析", async () => {
    mockedLabel.mockResolvedValue("DeepSeek");
    const { result, rerender } = renderHook(
      ({ p }: { p: Protocol | undefined }) => useProtocolLabel(p, "zh-Hans"),
      { initialProps: { p: undefined } },
    );
    expect(result.current).toBe("");
    expect(mockedLabel).not.toHaveBeenCalled();
    rerender({ p: "deepseek" });
    expect(result.current).toBe(""); // 解析落地前首渲染仍空
    await waitFor(() => expect(result.current).toBe("DeepSeek"));
  });
});

describe("useClientTypeOptions（票 11 clientTypeOptions 维度）", () => {
  beforeEach(() => vi.clearAllMocks());

  it("首渲染 []；resolve 后返回列表；reject 保空列表不崩", async () => {
    const list = [{ value: "codex_tui" as const, group: "OpenAI", label: "Codex TUI" }];
    mockedClientTypes.mockResolvedValueOnce(list);
    const first = renderHook(() => useClientTypeOptions("en-US"));
    expect(first.result.current).toEqual([]);
    await waitFor(() => expect(first.result.current).toEqual(list));
    first.unmount();

    mockedClientTypes.mockRejectedValueOnce(new Error("rpc fail"));
    const second = renderHook(() => useClientTypeOptions("en-US"));
    expect(second.result.current).toEqual([]);
    await act(async () => { await vi.waitFor(() => expect(mockedClientTypes).toHaveBeenCalledTimes(2)); });
    expect(second.result.current).toEqual([]); // best-effort：catch 后仍空列表
  });
});

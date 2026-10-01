// usePlatformForm 的 jev 决策槽：保存 payload 剔除空白槽、一键填充不进 jev。
import { describe, it, expect, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useRef } from "react";
import { usePlatformForm } from "./usePlatformForm";

function useForm() {
  const platformsEpochRef = useRef(0);
  const groupsReloadRef = useRef<(() => void) | null>(null);
  const consumedEditPidRef = useRef<number | null>(null);
  const deps = {
    t: ((k: string, f?: string) => f ?? k) as any,
    platforms: [],
    setPlatforms: vi.fn(),
    platformsEpochRef,
    quota: {} as any,
    setGroupDetails: vi.fn(),
    handleGroupsChanged: vi.fn(),
    groupsReloadRef,
    setToast: vi.fn(),
    breakerDefaults: {} as any,
    setUsageMap: vi.fn(),
    setLastTestMap: vi.fn(),
    onNavigate: vi.fn(),
    consumedEditPidRef,
  };
  return usePlatformForm(deps as any);
}

describe("usePlatformForm jev slot", () => {
  it("buildModelsPayload keeps jev, drops blank slots", () => {
    const { result } = renderHook(() => useForm());
    act(() => {
      result.current.setModels({ default: "", sonnet: "  ", opus: "", haiku: "", gpt: "", jev: " jev-latest " });
    });
    expect(result.current.buildModelsPayload()).toEqual({
      default: undefined, sonnet: undefined, opus: undefined, haiku: undefined, gpt: undefined,
      jev: "jev-latest",
    });
  });

  it("buildModelsPayload is undefined when every slot (incl. jev) is blank", () => {
    const { result } = renderHook(() => useForm());
    act(() => {
      result.current.setModels({ default: "", sonnet: "", opus: "", haiku: "", gpt: "", jev: "   " });
    });
    expect(result.current.buildModelsPayload()).toBeUndefined();
  });

  it("handleFillAll fills chat slots but not jev", () => {
    const { result } = renderHook(() => useForm());
    act(() => {
      result.current.setModels({ default: "glm-5", sonnet: "", opus: "", haiku: "", gpt: "", jev: "" });
    });
    act(() => { result.current.handleFillAll(); });
    expect(result.current.models).toEqual({
      default: "glm-5", sonnet: "glm-5", opus: "glm-5", haiku: "glm-5", gpt: "glm-5", jev: "",
    });
  });
});

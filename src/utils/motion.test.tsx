import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useInView, useReveal, useCounter, makeRipple } from "./motion";

// 动效 hooks 全部依赖 IntersectionObserver。jsdom 不实现它，所以每个测试自己装一个
// 假实现并保留回调句柄，用来手动模拟「进入视口」。缺 IO 的降级路径（直接激活）
// 通过删掉全局对象来测 —— Tauri webview 一定有 IO，但 SSR/旧 webview 会走这条。

type IOCallback = (entries: { isIntersecting: boolean }[]) => void;

let callbacks: IOCallback[] = [];
let disconnects = 0;

class FakeIO {
  constructor(cb: IOCallback) {
    callbacks.push(cb);
  }
  observe() {}
  disconnect() {
    disconnects++;
  }
}

const enterView = () => act(() => callbacks.forEach((cb) => cb([{ isIntersecting: true }])));
const leaveView = () => act(() => callbacks.forEach((cb) => cb([{ isIntersecting: false }])));

beforeEach(() => {
  callbacks = [];
  disconnects = 0;
  vi.stubGlobal("IntersectionObserver", FakeIO);
  // jsdom 的 requestAnimationFrame 传的 timestamp 与 performance.now() 不同基准，
  // useCounter 用 performance.now() 取起点会算出负进度。补一个同基准的实现。
  vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) =>
    setTimeout(() => cb(performance.now()), 1) as unknown as number,
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("useInView", () => {
  it("默认 true（首帧尚未 observe 时不让动画消失）", () => {
    const { result } = renderHook(() => useInView({ current: null }));
    expect(result.current).toBe(true);
  });

  it("进出视口双向切换", () => {
    const ref = { current: document.createElement("div") };
    const { result } = renderHook(() => useInView(ref));
    leaveView();
    expect(result.current).toBe(false);
    enterView();
    expect(result.current).toBe(true);
  });

  it("卸载时 disconnect", () => {
    const ref = { current: document.createElement("div") };
    const { unmount } = renderHook(() => useInView(ref));
    unmount();
    expect(disconnects).toBe(1);
  });

  it("无 IntersectionObserver 时保持 true 且不注册观察", () => {
    vi.stubGlobal("IntersectionObserver", undefined);
    const ref = { current: document.createElement("div") };
    const { result } = renderHook(() => useInView(ref));
    expect(result.current).toBe(true);
    expect(callbacks).toHaveLength(0);
  });
});

describe("useReveal", () => {
  it("初始未显示，进入视口后显示并 disconnect", () => {
    const { result } = renderHook(() => useReveal());
    act(() => {
      result.current.ref.current = document.createElement("div");
    });
    // ref 在首个 effect 后才有值，重挂一次让 effect 拿到元素
    const { result: r2 } = renderHook(() => {
      const h = useReveal();
      h.ref.current ??= document.createElement("div");
      return h;
    });
    expect(r2.current.shown).toBe(false);
    enterView();
    expect(r2.current.shown).toBe(true);
    expect(disconnects).toBeGreaterThan(0);
  });

  it("staggerMs > 0 时延迟激活", () => {
    vi.useFakeTimers();
    const { result } = renderHook(() => {
      const h = useReveal(300);
      h.ref.current ??= document.createElement("div");
      return h;
    });
    enterView();
    expect(result.current.shown).toBe(false);
    act(() => void vi.advanceTimersByTime(300));
    expect(result.current.shown).toBe(true);
  });

  it("未进入视口时不显示", () => {
    const { result } = renderHook(() => {
      const h = useReveal();
      h.ref.current ??= document.createElement("div");
      return h;
    });
    leaveView();
    expect(result.current.shown).toBe(false);
  });

  it("无 IntersectionObserver 时直接显示", () => {
    vi.stubGlobal("IntersectionObserver", undefined);
    const { result } = renderHook(() => {
      const h = useReveal();
      h.ref.current ??= document.createElement("div");
      return h;
    });
    expect(result.current.shown).toBe(true);
  });
});

describe("useCounter", () => {
  it("初始显示目标值本身（decimals 决定精度）", () => {
    const { result } = renderHook(() => useCounter(42));
    expect(result.current.display).toBe("42");
    const { result: dec } = renderHook(() => useCounter(1.5, 2));
    expect(dec.current.display).toBe("1.50");
  });

  it("无 IntersectionObserver 时立即启动并缓动到目标值", async () => {
    vi.stubGlobal("IntersectionObserver", undefined);
    const { result } = renderHook(() => {
      const h = useCounter(100, 0, 10);
      h.ref.current ??= document.createElement("span");
      return h;
    });
    await act(() => new Promise((r) => setTimeout(r, 60)));
    expect(Number(result.current.display.replace(/,/g, ""))).toBe(100);
  });

  it("进入视口后启动，且重复进入只启动一次", async () => {
    const { result } = renderHook(() => {
      const h = useCounter(50, 1, 10);
      h.ref.current ??= document.createElement("span");
      return h;
    });
    enterView();
    enterView();
    await act(() => new Promise((r) => setTimeout(r, 60)));
    expect(result.current.display).toBe("50.0");
  });
});

describe("makeRipple", () => {
  const clickOn = (el: HTMLElement) =>
    makeRipple({
      currentTarget: el,
      clientX: 50,
      clientY: 20,
    } as unknown as React.MouseEvent<HTMLElement>);

  it("生成一个尺寸取长边的 .ripple-wave，600ms 后移除", () => {
    vi.useFakeTimers();
    const el = document.createElement("button");
    el.getBoundingClientRect = () =>
      ({ width: 80, height: 40, left: 10, top: 5 }) as DOMRect;

    clickOn(el);
    const wave = el.querySelector<HTMLElement>(".ripple-wave");
    expect(wave).not.toBeNull();
    expect(wave!.style.width).toBe("80px");
    expect(wave!.style.left).toBe("0px"); // 50 - 10 - 40
    expect(wave!.style.top).toBe("-25px"); // 20 - 5 - 40

    vi.advanceTimersByTime(600);
    expect(el.querySelector(".ripple-wave")).toBeNull();
  });

  it("已有波纹时不重复生成", () => {
    vi.useFakeTimers();
    const el = document.createElement("button");
    el.getBoundingClientRect = () => ({ width: 10, height: 10, left: 0, top: 0 }) as DOMRect;
    clickOn(el);
    clickOn(el);
    expect(el.querySelectorAll(".ripple-wave")).toHaveLength(1);
  });
});

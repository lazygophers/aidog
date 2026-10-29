import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

const invoke = vi.hoisted(() => vi.fn());
const isTauri = vi.hoisted(() => vi.fn(() => true));
vi.mock("../services/transport", () => ({ invoke, isTauri }));

const LEVELS = ["error", "warn", "info", "debug"] as const;
const con = console as unknown as Record<string, unknown>;
const saved = Object.fromEntries(LEVELS.map((l) => [l, con[l]]));
const stubs = LEVELS.map((l) => [l, vi.fn()] as const);

async function load() {
  vi.resetModules();
  return (await import("./frontendLog")).installFrontendLog;
}

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue(undefined);
  isTauri.mockReturnValue(true);
  // 让 install 绑定到可观察的 stub，避免测试输出污染
  for (const [l, f] of stubs) {
    f.mockClear();
    con[l] = f;
  }
});
afterEach(() => {
  for (const l of LEVELS) con[l] = saved[l];
});

describe("installFrontendLog", () => {
  it("console 各级别转发到 app_log_frontend，并仍调原 console", async () => {
    const install = await load();
    install();
    console.warn("a", { x: 1 }, 3);
    expect(stubs[1][1]).toHaveBeenCalledWith("a", { x: 1 }, 3);
    expect(invoke).toHaveBeenCalledWith("app_log_frontend", { level: "warn", message: 'a {"x":1} 3' });
    console.error("e");
    console.info("i");
    console.debug("d");
    expect(invoke.mock.calls.map((c) => c[1].level)).toEqual(["warn", "error", "info", "debug"]);
  });

  it("循环引用对象退 String()", async () => {
    const install = await load();
    install();
    const o: Record<string, unknown> = {};
    o.self = o;
    console.info(o);
    expect(invoke.mock.calls[0][1].message).toBe("[object Object]");
  });

  it("invoke reject 被静默吞掉", async () => {
    invoke.mockRejectedValue(new Error("boom"));
    const install = await load();
    install();
    expect(() => console.error("x")).not.toThrow();
    await Promise.resolve();
  });

  it("非 Tauri 环境：不 patch console、不 invoke", async () => {
    isTauri.mockReturnValue(false);
    const before = console.error;
    const install = await load();
    install();
    expect(console.error).toBe(before);
    console.error("x");
    expect(invoke).not.toHaveBeenCalled();
  });

  it("重复调用只装一次", async () => {
    const install = await load();
    install();
    install();
    console.error("once");
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it("window error / unhandledrejection 转发为 error 级", async () => {
    const install = await load();
    install();
    window.dispatchEvent(new ErrorEvent("error", { message: "bad", filename: "f.js", lineno: 7 }));
    expect(invoke.mock.calls.at(-1)![1].message).toBe("uncaught: bad @ f.js:7");
    const ev = new Event("unhandledrejection") as Event & { reason: unknown };
    ev.reason = "why";
    window.dispatchEvent(ev);
    expect(invoke.mock.calls.at(-1)![1]).toEqual({ level: "error", message: "unhandledrejection: why" });
  });
});

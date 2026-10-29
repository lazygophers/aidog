import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn(() => Promise.resolve());
const isTauri = vi.fn(() => false);
vi.mock("../services/transport", () => ({ invoke, isTauri }));

const saved = { ...console };

beforeEach(() => {
  vi.resetModules();
  invoke.mockClear();
});
afterEach(() => {
  Object.assign(console, saved);
});

describe("installFrontendLog", () => {
  it("非 Tauri 环境：不 patch console、不 invoke app_log_frontend", async () => {
    isTauri.mockReturnValue(false);
    const before = console.error;
    const { installFrontendLog } = await import("./frontendLog");
    installFrontendLog();
    expect(console.error).toBe(before);
    expect(invoke).not.toHaveBeenCalled();
  });

  it("Tauri 环境：patch console 并转发到 app_log_frontend", async () => {
    isTauri.mockReturnValue(true);
    const orig = vi.spyOn(console, "warn").mockImplementation(() => {});
    const { installFrontendLog } = await import("./frontendLog");
    installFrontendLog();
    console.warn("hi", { a: 1 });
    expect(invoke).toHaveBeenCalledWith("app_log_frontend", {
      level: "warn",
      message: 'hi {"a":1}',
    });
    orig.mockRestore();
  });
});

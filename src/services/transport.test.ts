// transport.test.ts — 传输层分流（票 09）。
//
// 覆盖三件事：
// 1. 探测 `window.__TAURI_INTERNALS__` 选中正确实现（有 → Tauri IPC，无 → HTTP）；
// 2. 两条路的参数形状一致（同一次调用，Tauri 侧收到的 args 与 HTTP body 逐字相同）；
// 3. 错误值形状一致（Tauri reject 值 == HTTP 非 2xx body）。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import {
  invoke,
  listen,
  isTauri,
  RPC_UNAUTHORIZED,
  __resetEventSourceForTests,
} from "./transport";

/** 最小 EventSource 桩：只实现 transport 用到的那几个成员。 */
class FakeEventSource {
  static last: FakeEventSource | null = null;
  url: string;
  onerror: (() => void) | null = null;
  closed = false;
  listeners = new Map<string, Set<EventListener>>();

  constructor(url: string) {
    this.url = url;
    FakeEventSource.last = this;
  }
  addEventListener(name: string, fn: EventListener) {
    if (!this.listeners.has(name)) this.listeners.set(name, new Set());
    this.listeners.get(name)!.add(fn);
  }
  removeEventListener(name: string, fn: EventListener) {
    this.listeners.get(name)?.delete(fn);
  }
  close() {
    this.closed = true;
  }
  /** 模拟内核推一条具名 SSE 事件。 */
  push(name: string, data: string) {
    this.listeners.get(name)?.forEach((fn) => fn({ data } as MessageEvent));
  }
}

const ARGS = { platformId: 7, filter: { model: "gpt-5" }, limit: 50 };

describe("transport 分流", () => {
  beforeEach(() => {
    // `clearMocks()` 只删 `__TAURI_INTERNALS__` 的成员、留着空壳对象，探测标志因此仍为
    // 真。要测「非 Tauri 分支」必须把整个对象删掉。
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    __resetEventSourceForTests();
    FakeEventSource.last = null;
    (globalThis as unknown as { EventSource: unknown }).EventSource = FakeEventSource;
  });

  afterEach(() => {
    clearMocks();
    vi.unstubAllGlobals();
    __resetEventSourceForTests();
  });

  it("Tauri 存在时走 IPC，不发 HTTP 请求", async () => {
    const fetchSpy = vi.fn();
    vi.stubGlobal("fetch", fetchSpy);
    const seen: Array<{ cmd: string; args: unknown }> = [];
    mockIPC((cmd, args) => {
      seen.push({ cmd, args });
      return "ok";
    });

    expect(isTauri()).toBe(true);
    await expect(invoke<string>("proxy_log_list", ARGS)).resolves.toBe("ok");
    expect(seen).toEqual([{ cmd: "proxy_log_list", args: ARGS }]);
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it("Tauri 不存在时 POST /rpc/<命令>，body 就是 args", async () => {
    const fetchSpy = vi.fn().mockResolvedValue(
      new Response(JSON.stringify([{ id: "a" }]), { status: 200 }),
    );
    vi.stubGlobal("fetch", fetchSpy);

    expect(isTauri()).toBe(false);
    await expect(invoke("proxy_log_list", ARGS)).resolves.toEqual([{ id: "a" }]);

    const [url, init] = fetchSpy.mock.calls[0];
    expect(url).toBe("/rpc/proxy_log_list");
    expect(init.method).toBe("POST");
    expect(init.headers).toEqual({ "Content-Type": "application/json" });
    // 参数形状与 Tauri 路一致（同一个对象原样 JSON 化，键名不做任何转换）。
    expect(JSON.parse(init.body)).toEqual(ARGS);
  });

  it("无 args 时 HTTP body 是空对象（对齐 Tauri 的 args 缺省）", async () => {
    const fetchSpy = vi.fn().mockResolvedValue(new Response("null", { status: 200 }));
    vi.stubGlobal("fetch", fetchSpy);

    await expect(invoke("proxy_stop")).resolves.toBeNull();
    expect(JSON.parse(fetchSpy.mock.calls[0][1].body)).toEqual({});
  });

  it("HTTP 非 2xx 抛出 body 本身，与 Tauri reject 值同形", async () => {
    // Tauri 路：Rust `Err(ProxyStartError)` 原样 reject。
    mockIPC(() => {
      throw { kind: "addr_in_use", port: 8080 };
    });
    const tauriErr = await invoke("proxy_start", { port: 8080 }).catch((e) => e);
    clearMocks();
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;

    // HTTP 路：同一个值落在非 2xx 响应 body 里。
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(JSON.stringify({ kind: "addr_in_use", port: 8080 }), { status: 500 }),
      ),
    );
    const httpErr = await invoke("proxy_start", { port: 8080 }).catch((e) => e);

    expect(httpErr).toEqual(tauriErr);
    expect(httpErr).toEqual({ kind: "addr_in_use", port: 8080 });
  });

  it("401 被认出来是未授权，而不是 JSON 解析报错", async () => {
    // 内核 `server.rs::UNAUTHORIZED_BODY` 回的就是这段 JSON 字符串字面量。
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response('"unauthorized"', {
          status: 401,
          headers: { "Content-Type": "application/json" },
        }),
      ),
    );
    const err = await invoke("about_info").catch((e) => e);
    expect(err).toBe(RPC_UNAUTHORIZED);
    expect(err).not.toBeInstanceOf(SyntaxError);
  });

  it("非 JSON 的错误响应抛原文，不被 SyntaxError 顶替", async () => {
    // 反向代理自己的错误页 / 静态资源 fallback 都不是 JSON。
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(new Response("<html>502 Bad Gateway</html>", { status: 502 })),
    );
    const err = await invoke("about_info").catch((e) => e);
    expect(err).toBe("<html>502 Bad Gateway</html>");
  });

  it("Tauri 存在时 listen 走事件总线，不开 SSE", async () => {
    mockIPC(() => 1);
    const un = await listen("proxy-log-updated", () => {});
    expect(FakeEventSource.last).toBeNull();
    un();
  });

  it("Tauri 不存在时 listen 订阅 /events 的具名 SSE 事件，payload 与 emit 同形", async () => {
    const seen: unknown[] = [];
    const un = await listen<number>("proxy-log-updated", (e) => seen.push(e.payload));

    const es = FakeEventSource.last!;
    expect(es.url).toBe("/events");
    es.push("proxy-log-updated", "3");
    es.push("other-event", "9");
    expect(seen).toEqual([3]);

    // unlisten 后不再收；同一 JS 上下文只开一条连接。
    un();
    es.push("proxy-log-updated", "4");
    expect(seen).toEqual([3]);

    await listen("aidog-deep-link", () => {});
    expect(FakeEventSource.last).toBe(es);
  });

  it("坏 SSE 帧被跳过，不影响后续事件", async () => {
    const seen: unknown[] = [];
    await listen<number>("proxy-log-updated", (e) => seen.push(e.payload));
    const es = FakeEventSource.last!;
    es.push("proxy-log-updated", "not json");
    es.push("proxy-log-updated", "5");
    expect(seen).toEqual([5]);
  });
});

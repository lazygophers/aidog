// transport.ts — 前端传输层分流（票 09）。
//
// aidog 的前端有两种壳：
//
// 1. **桌面版**（Tauri）：命令走 Tauri IPC（`invoke`），事件走 Tauri 事件总线（`listen`）。
// 2. **浏览器版**（票 08 的 `aidog-kernel --ui` 托管同一份 dist）：命令走
//    `POST /rpc/<命令名>`（args 作 JSON body），事件走 SSE `/events`。
//
// 分流依据是运行时探测 `window.__TAURI_INTERNALS__`（Tauri v2 注入到 webview 的内部对象，
// `@tauri-apps/api` 自己也是读它）。**每次调用都重新探测**，不在模块加载时定死：
// 单测里的 `mockIPC()` 是在模块加载之后才注入这个对象的。
//
// 两条路的签名严格对齐，调用方（`services/api/*.ts`）只换 import 路径，代码一行不改：
//
// - `invoke<T>(cmd, args)` → `Promise<T>`；失败时 reject 的值两条路一致
//   （Tauri 是 Rust `Err(e)` 的 JSON，HTTP 是非 2xx 响应的 JSON body，
//   由 `aidog_core::http_command` 保证是同一个值）。
// - `listen<T>(event, handler)` → `Promise<UnlistenFn>`。
//
// 参数键名两条路也一致：Tauri v2 按 lowerCamelCase 取参，内核的 HTTP 形态
// （`http_command.rs::extract_arg`）先按 camelCase 取、取不到再按 snake_case 取，
// 所以前端发什么都不用改。

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen, type Event as TauriEvent, type UnlistenFn } from "@tauri-apps/api/event";

export type { UnlistenFn };

/** 事件回调签名（与 `@tauri-apps/api/event` 的 `EventCallback<T>` 对齐）。 */
export type EventCallback<T> = (event: TauriEvent<T>) => void;

/** 管理面 RPC 前缀，与 `aidog_kernel::rpc::rpc_router` 的路由一致。 */
const RPC_PREFIX = "/rpc/";

/** 管理面 SSE 端点，与 `aidog_kernel::server::management_router` 的路由一致。 */
const EVENTS_PATH = "/events";

/**
 * 当前是否跑在 Tauri webview 里。
 *
 * 惰性判断（每次调用重新读），不要缓存：`mockIPC()` 会在模块加载之后注入该对象。
 */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/**
 * 未授权时管理面 reject 的值（内核 `server.rs::UNAUTHORIZED_BODY`）。
 *
 * 配了访问令牌但请求没带对 Bearer 时，`invoke` 抛的就是这个字符串。判断方式：
 * `e === RPC_UNAUTHORIZED`。
 */
export const RPC_UNAUTHORIZED = "unauthorized";

/** HTTP 形态的命令调用：`POST /rpc/<cmd>`，body = args。 */
async function httpInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const res = await fetch(`${RPC_PREFIX}${cmd}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(args ?? {}),
  });
  // 无返回值的命令（Rust `()`）body 是 "null"；空 body 也按 null 处理。
  const text = await res.text();
  let data: unknown = null;
  if (text.length > 0) {
    try {
      data = JSON.parse(text);
    } catch {
      // 这是网络边界，回什么都可能：命中静态资源 fallback 拿到 index.html、被反向代理
      // 拦下拿到它自己的错误页，都不是 JSON。此时把原文当错误值抛出去，别让 SyntaxError
      // 顶替掉真正的失败原因（真错误被掩盖过一次，见 UNAUTHORIZED_BODY 的注释）。
      throw text;
    }
  }
  if (!res.ok) {
    // 与 Tauri 的 reject 对齐：抛出的就是错误值本身（`Result<_, String>` 即字符串，
    // 结构化错误即对象 —— `isProxyStartError` 这类类型守卫两条路都成立）。
    throw data;
  }
  return data as T;
}

/**
 * 调命令。桌面版走 Tauri IPC，浏览器版走 `POST /rpc/<cmd>`。
 *
 * 签名与 `@tauri-apps/api/core` 的 `invoke` 一致，`services/api/*.ts` 直接换 import 即可。
 */
export function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri()) return tauriInvoke<T>(cmd, args);
  return httpInvoke<T>(cmd, args);
}

// ─── SSE 事件（浏览器形态） ──────────────────────────────────
// 整个 JS 上下文共用一条 `/events` 连接，按事件名 `addEventListener` 扇出。
// 这与 Tauri 侧的语义一致：内核把 `emit(name, payload)` 原样广播成 SSE 具名事件
// （`aidog_kernel::server::event_stream`），data 就是 payload 的 JSON 原文。

let eventSource: EventSource | null = null;

function ensureEventSource(): EventSource {
  if (!eventSource) {
    eventSource = new EventSource(EVENTS_PATH);
    // EventSource 自带重连（内核重启 / 网络抖动后自动恢复），这里只记一条日志，
    // 不销毁实例 —— 销毁反而会掐掉浏览器的自动重连。
    eventSource.onerror = () => {
      console.warn("[transport] /events stream interrupted, browser will retry");
    };
  }
  return eventSource;
}

/** 测试用：丢弃当前 SSE 连接。生产代码不要调。 */
export function __resetEventSourceForTests(): void {
  eventSource?.close();
  eventSource = null;
}

function httpListen<T>(event: string, handler: EventCallback<T>): Promise<UnlistenFn> {
  const source = ensureEventSource();
  const wrapped = (e: MessageEvent<string>) => {
    let payload: T;
    try {
      payload = JSON.parse(e.data) as T;
    } catch {
      // keep-alive 注释帧不会走到这里（EventSource 自己吃掉），能到这里的是坏帧，跳过。
      console.warn("[transport] dropped malformed SSE frame", event);
      return;
    }
    handler({ event, id: 0, payload });
  };
  source.addEventListener(event, wrapped as EventListener);
  return Promise.resolve(() => source.removeEventListener(event, wrapped as EventListener));
}

/**
 * 订阅后端事件。桌面版走 Tauri 事件总线，浏览器版走内核 SSE `/events`。
 *
 * 签名与 `@tauri-apps/api/event` 的 `listen` 一致。
 */
export function listen<T>(event: string, handler: EventCallback<T>): Promise<UnlistenFn> {
  if (isTauri()) return tauriListen<T>(event, handler);
  return httpListen<T>(event, handler);
}

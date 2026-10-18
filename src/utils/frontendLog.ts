/**
 * 前端 console 统一日志接入（unified-logging 票 02）。
 *
 * webview 的 console 输出用完即丢，GUI 卡顿/报错无从回看。本模块在入口处
 * monkey-patch console（error/warn/info/debug），把每条转发给 Rust 侧
 * `app_log_frontend` 命令，与后端 tracing 落同一个 ~/.aidog/logs/aidog.<小时>.log
 * （按小时分割、按 retention_hours 清理）。
 *
 * 约束：
 * - 防递归：转发失败（invoke 抛错）时静默吞——绝不能走 console.error 再触发自己。
 * - 非 Tauri 环境（vite dev 纯浏览器 / vitest）不装 patch，console 行为不变。
 * - 参数序列化尽量短：对象 JSON.stringify 失败退 String()，超长由 Rust 侧截断。
 */
import { invoke, isTauri } from "../services/transport";

let installed = false;

function toMessage(args: unknown[]): string {
  return args
    .map((a) => {
      if (typeof a === "string") return a;
      try {
        return JSON.stringify(a);
      } catch {
        return String(a);
      }
    })
    .join(" ");
}

function forward(level: string, args: unknown[]) {
  // fire-and-forget：catch 里什么都不做（防递归），丢一条日志无害
  invoke("app_log_frontend", { level, message: toMessage(args) }).catch(() => {});
}

export function installFrontendLog() {
  if (installed || !isTauri) return;
  installed = true;
  for (const level of ["error", "warn", "info", "debug"] as const) {
    const orig = console[level].bind(console);
    console[level] = (...args: unknown[]) => {
      orig(...args);
      forward(level, args);
    };
  }
  // 未捕获异常 / Promise 拒绝：GUI 白屏/卡死的第一手线索
  window.addEventListener("error", (e) => {
    forward("error", [
      `uncaught: ${e.message} @ ${e.filename ?? "?"}:${e.lineno ?? 0}`,
    ]);
  });
  window.addEventListener("unhandledrejection", (e) => {
    forward("error", [`unhandledrejection: ${String(e.reason)}`]);
  });
}

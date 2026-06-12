import { useEffect, useRef } from "react";

/**
 * 可见性感知轮询：仅当页面可见 (document.visibilityState === "visible") 时运行回调。
 * 页面切到后台时暂停定时器，回到前台时立即触发一次并恢复轮询。
 *
 * - callback 用 ref 持有最新闭包，依赖变化不会重启定时器（除非 intervalMs / enabled 变）
 * - enabled=false 时完全停止
 *
 * @param callback 每个 tick 执行的函数（前台可见时）
 * @param intervalMs 轮询间隔（毫秒）
 * @param enabled 是否启用轮询，默认 true
 */
export function usePolling(
  callback: () => void,
  intervalMs: number,
  enabled: boolean = true,
): void {
  const savedCallback = useRef(callback);

  useEffect(() => {
    savedCallback.current = callback;
  }, [callback]);

  useEffect(() => {
    if (!enabled) return;

    let timer: ReturnType<typeof setInterval> | null = null;

    const tick = () => {
      if (document.visibilityState === "visible") {
        savedCallback.current();
      }
    };

    const start = () => {
      if (timer === null) {
        timer = setInterval(tick, intervalMs);
      }
    };

    const stop = () => {
      if (timer !== null) {
        clearInterval(timer);
        timer = null;
      }
    };

    const onVisibilityChange = () => {
      if (document.visibilityState === "visible") {
        // 回到前台：立即刷新一次再恢复轮询
        savedCallback.current();
        start();
      } else {
        stop();
      }
    };

    if (document.visibilityState === "visible") start();
    document.addEventListener("visibilitychange", onVisibilityChange);

    return () => {
      stop();
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  }, [intervalMs, enabled]);
}

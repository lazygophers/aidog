import { useEffect } from "react";
import { requestNavigation } from "./navGuard";

// 关窗请求 → 走 navGuard：有未保存改动时由页面的确认弹窗决定放行，无则直接销毁。
// 主窗口关闭 = 销毁（Rust 侧 `ui_window`），故必须在这里拦，销毁后草稿无法保留。
// 注册 JS 监听后 Tauri 会替我们 prevent_close，销毁一律由这里显式 `destroy()`
// （capabilities 需 core:window:allow-destroy）。
export function useCloseGuard(): void {
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    (async () => {
      try {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        const win = getCurrentWindow();
        const stop = await win.onCloseRequested((event) => {
          event.preventDefault();
          requestNavigation(() => void win.destroy());
        });
        if (cancelled) stop();
        else unlisten = stop;
      } catch {
        // 非 Tauri 环境（浏览器形态）→ 无窗口可关，跳过。
      }
    })();
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);
}

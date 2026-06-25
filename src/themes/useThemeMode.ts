import { useEffect, useState } from "react";
import type { ThemeMode } from "./types";
import { DEFAULT_MODE } from "./index";

/**
 * 订阅 documentElement 的 data-mode 属性（applyTheme 写入），主题切换时自动 re-render。
 * 轻量 hook：仅读 data-mode + MutationObserver 监听属性变化，不拉入 settings 上下文，
 * 避免给性能敏感页（Platforms）引入额外 re-render。
 *
 * 用于 datetime-local / date / time 等 `<input>` 的 colorScheme 属性 —— 该 CSS 属性控制
 * WKWebView 原生日历弹出层明暗（input 本体走 .input className 的 CSS 变量适配主题）。
 */
export function useThemeMode(): ThemeMode {
  const [mode, setMode] = useState<ThemeMode>(() => readMode());

  useEffect(() => {
    // 挂载时再读一次（防 cold-start 时 data-mode 尚未写入）。
    setMode(readMode());

    const root = document.documentElement;
    const observer = new MutationObserver(() => {
      setMode(readMode());
    });
    // 只监听 data-mode 属性变化（applyTheme 切主题时 setAttribute）。
    observer.observe(root, { attributes: true, attributeFilter: ["data-mode"] });
    return () => observer.disconnect();
  }, []);

  return mode;
}

function readMode(): ThemeMode {
  const v = document.documentElement.dataset.mode;
  return v === "light" || v === "dark" ? v : DEFAULT_MODE;
}

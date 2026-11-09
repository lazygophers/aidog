import { useCallback, useState } from "react";

const KEY = "aidog.lastNav";

// 导航页持久化：界面关闭即销毁 webview，重建时读回关窗前那一页（localStorage 随应用数据目录保留）。
// 读不到 / 不在合法清单 → 回落 fallback。草稿不持久化。
export function usePersistedNav(valid: string[], fallback: string, aliases: Record<string, string> = {}): [string, (id: string) => void] {
  const [nav, setNavState] = useState(() => {
    try {
      const v = localStorage.getItem(KEY);
      const id = v ? (aliases[v] ?? v) : null;
      return id && valid.includes(id) ? id : fallback;
    } catch {
      return fallback;
    }
  });
  const setNav = useCallback((id: string) => {
    setNavState(id);
    try {
      localStorage.setItem(KEY, id);
    } catch {
      // 存储不可用 → 只是不记住，不影响导航。
    }
  }, []);
  return [nav, setNav];
}

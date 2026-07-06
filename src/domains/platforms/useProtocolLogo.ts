import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { Protocol } from "../../services/api";
import { getProtocolLogoPath, syncProtocolLogo } from "../../services/api";

/** 解析 protocol logo 的 webview 可访问 URL。
 *
 *  流程：mount 时 `get_protocol_logo_path` 查缓存路径 →
 *  - 命中（非空）→ `convertFileSrc(path)` 设为 logoSrc，`<img>` 渲染。
 *  - miss（空串）→ 触发 `sync_protocol_logo` 后台异步下载（不阻塞，立即返 fallback），
 *    本会话不再轮询；下次 mount 命中即用。
 *  - `<img>` onError → 清 logoSrc fallback 首字母圆圈（防 broken image icon）。
 *
 *  ponytail: 单查询无状态机 / 无轮询。Rust 后台预热 + 用户重启会话覆盖绝大多数场景。 */
export function useProtocolLogo(protocol: Protocol): {
  logoSrc: string | null;
  fallbackInitial: string;
} {
  const [logoSrc, setLogoSrc] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLogoSrc(null);
    (async () => {
      try {
        const path = await getProtocolLogoPath(protocol);
        if (cancelled) return;
        if (path) {
          setLogoSrc(convertFileSrc(path));
        } else {
          // 缓存 miss：触发后台同步，本会话不等待（下次 mount 命中）
          syncProtocolLogo(protocol).catch((e) =>
            console.warn("[logo] syncProtocolLogo failed:", e),
          );
        }
      } catch (e) {
        console.warn("[logo] getProtocolLogoPath failed:", e);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [protocol]);

  const fallbackInitial = protocol ? protocol[0]!.toUpperCase() : "?";
  return { logoSrc, fallbackInitial };
}

import type { NextConfig } from "next";
import path from "node:path";

/**
 * 浏览器版（票 I14）：Next.js 静态导出，与桌面版共用同一份 `../src/`。
 *
 * 工程放在 `web/` 子目录而不是仓库根：Next 的路由扫描会把 React 版的 `src/pages/`
 * 误认成 Pages Router 目录（「pages and app directories should be under the same
 * folder」），根目录放 `app/` 与它天然冲突。
 *
 * - `turbopack.root` 显式指到仓库根（yarn.lock 所在）：Turbopack 只解析 root 之内的
 *   模块，`@aidog-i18n` 指到的 `src-tauri/crates/aidog_i18n` 才能被解析。
 * - `output: "export"`：纯静态产物到 `out/`，不要 SSR。内核 `aidog-kernel --ui --ui-dir out`
 *   直接托管（`aidog_kernel/src/server.rs::management_router` 带 SPA fallback）。
 * - `*.svg` asset 规则让静态 SVG import 返回 URL 字符串（= Vite 的默认行为），
 *   `src/` 里 14 处 `import x from "*.svg"` 零改动。
 * - 产物目录用 `out/` 而不是 `dist/`：`dist/` 是 Vite（桌面 Tauri 壳）的产物目录，
 *   两条构建链不写同一个目录；内核启动用 `--ui-dir web/out` 显式指定。
 */
const repoRoot = path.join(__dirname, "..");

const nextConfig: NextConfig = {
  output: "export",
  turbopack: {
    root: repoRoot,
    resolveAlias: {
      "@": path.join(repoRoot, "src"),
      "@aidog-i18n": path.join(repoRoot, "src-tauri/crates/aidog_i18n"),
    },
    rules: {
      "*.svg": { type: "asset" },
    },
  },
};

export default nextConfig;

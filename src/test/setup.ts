import "@testing-library/jest-dom/vitest";
import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";
import { clearMocks } from "@tauri-apps/api/mocks";

// 纯函数测试文件用 `// @vitest-environment node`（无 window/DOM），此时
// cleanup/clearMocks 不适用且会抛 `window is not defined`，按 window 存在性守卫。
afterEach(() => {
  if (typeof window === "undefined") return;
  cleanup(); // 卸载 RTL 渲染的组件
  clearMocks(); // 清 Tauri IPC mock，防跨用例串味
});

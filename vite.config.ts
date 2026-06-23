/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],

  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    rollupOptions: {
      input: {
        main: "./index.html",
        popover: "./popover.html",
      },
    },
  },
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    css: false,
    include: ["src/**/*.{test,spec}.{ts,tsx}"],
    coverage: {
      provider: "v8",
      reporter: ["text", "text-summary", "html", "lcov"],
      reportsDirectory: "./coverage",
      // 仅统计已建立测试覆盖的纯逻辑层 + 共享展示组件 + 服务封装。
      // 巨石页面 / 编排容器（Platforms/Groups/Logs/Settings/editors 等）IPC fan-out +
      // 乐观更新 + epoch 守卫，全量渲染测成本极高且脆，按 PRD §6 决策不纳入分母
      // （避免为薄编排壳硬凑行覆盖）。其纯函数逻辑已抽至 utils/shared，单独测覆盖。
      include: [
        "src/utils/**/*.ts",
        "src/components/shared/**/*.{ts,tsx}",
        "src/services/api.ts",
      ],
      exclude: [
        "src/**/*.{test,spec}.{ts,tsx}",
        "src/test/**",
      ],
      thresholds: {
        branches: 80,
        functions: 80,
        lines: 80,
        statements: 80,
      },
    },
  },
}));

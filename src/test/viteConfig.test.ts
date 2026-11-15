// @vitest-environment node
import { resolveConfig } from "vite";
import { expect, test } from "vitest";

// 内置 /ui 页面靠相对 base 加载资源；被忽略的旧 vite.config.js 会压过 .ts，使产物写成绝对 /assets。
test("vite 解析到 vite.config.ts 且 base 为相对路径", async () => {
  const cfg = await resolveConfig({ root: process.cwd() }, "build");
  expect(cfg.configFile).toMatch(/vite\.config\.ts$/);
  expect(cfg.base).toBe("./");
});

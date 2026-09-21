#!/usr/bin/env node
// 把品牌图标同步进 Flutter 壳。真值源只有一个：`src-tauri/icons/`（Tauri 壳在用的那套）。
//
// 产出两样：
//   1. macOS AppIcon 的 7 个尺寸 —— 不生成的话打出来的包还挂着 Flutter 模板那个蓝色 F。
//   2. `flutter/assets/logo.webp` —— 顶栏品牌标，对应 React 的 `<img src="/logo.svg">`。
//      React 那个 .svg 其实是包了一层 svg 壳的 base64 webp，所以这里直接用同一张 webp 原图。
//
// 用法：
//   node scripts/gen-flutter-icons.mjs           # 生成
//   node scripts/gen-flutter-icons.mjs --check   # 只校验，漂了就非零退出（给 make lint 用）

import { execFileSync } from "node:child_process";
import { copyFileSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const check = process.argv.includes("--check");
const repo = new URL("..", import.meta.url).pathname;
const icns = join(repo, "src-tauri/icons/icon.icns");
const logoSrc = join(repo, "src-tauri/icons/source/logo.webp");
const iconDir = join(repo, "flutter/macos/Runner/Assets.xcassets/AppIcon.appiconset");
const logoDst = join(repo, "flutter/assets/logo.webp");

// AppIcon 的目标尺寸 → iconutil 导出的哪一张。@2x 那几张的像素尺寸正好是翻倍后的值。
const MAP = {
  16: "icon_16x16.png",
  32: "icon_32x32.png",
  64: "icon_32x32@2x.png",
  128: "icon_128x128.png",
  256: "icon_256x256.png",
  512: "icon_512x512.png",
  1024: "icon_512x512@2x.png",
};

const work = mkdtempSync(join(tmpdir(), "aidog-icons-"));
const iconset = join(work, "icon.iconset");
execFileSync("iconutil", ["-c", "iconset", icns, "-o", iconset]);

const drift = [];
const write = (dst, src) => {
  if (check) {
    const a = readFileSync(dst);
    const b = readFileSync(src);
    if (!a.equals(b)) drift.push(dst.replace(repo, ""));
    return;
  }
  copyFileSync(src, dst);
};

for (const [size, name] of Object.entries(MAP)) {
  write(join(iconDir, `app_icon_${size}.png`), join(iconset, name));
}
write(logoDst, logoSrc);
rmSync(work, { recursive: true, force: true });

if (check) {
  if (drift.length) {
    console.error(`[flutter-icons] 与 src-tauri/icons/ 不一致，请跑 node scripts/gen-flutter-icons.mjs：`);
    for (const f of drift) console.error(`  ${f}`);
    process.exit(1);
  }
  console.log("[flutter-icons] ok");
} else {
  console.log(`[flutter-icons] 写出 ${Object.keys(MAP).length} 个 AppIcon 尺寸 + assets/logo.webp`);
}

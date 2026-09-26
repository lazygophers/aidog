#!/usr/bin/env node
// 护栏：Flutter 侧不许出现「Material 默认下划线」。
//
// 两类来源，用户 2026-09-21 明确要求消掉（原话：「不要有那么多下划线，
// 我不希望设计上有这么多下划线设计，很丑」）：
//   1. `DropdownButton` 底下那条线 —— 必须 `underline:` 传空，或外面包
//      `DropdownButtonHideUnderline`。
//   2. `TextField` 的 underline 边框 —— `theme.dart` 的 `inputDecorationTheme`
//      全局给显式边框（b78275771 批次二 P1：InputBorder.none 裸文字行改描边盒，
//      对齐 React `.input`）。缺了这套显式 border，TextField 会回落 Material
//      默认 underline；这里守住整套配置不被删。
//
// 还有第三类不在本脚本范围：文字被画上**黄色**双下划线，那是「缺 Material 祖先」
// 的运行时提示，不是样式（修法见 `app_shell.dart` 与 `popover/app.dart` 的注释）。

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { execFileSync } from "node:child_process";

const root = new URL("..", import.meta.url).pathname;
const bad = [];

// ── 1. 裸 DropdownButton ──
const files = execFileSync("grep", ["-rl", "DropdownButton", join(root, "flutter/lib")], {
  encoding: "utf8",
}).trim().split("\n").filter(Boolean);

for (const f of files) {
  const src = readFileSync(f, "utf8");
  const lines = src.split("\n");
  lines.forEach((line, i) => {
    if (!/\bDropdownButton</.test(line)) return;
    // 扫到该构造器的收尾为止，不用固定行数——`DropdownButton` 的参数表长短不一，
    // 卡死 N 行会把 `underline:` 写在后面的写法误报成缺失（第一版就踩了）。
    const indent = line.search(/\S/);
    let end = i + 1;
    while (end < lines.length && end < i + 80) {
      const l = lines[end];
      // 同缩进层级上的 `)` / `);` / `),` 即本构造器收尾。
      if (l.trim() !== "" && l.search(/\S/) <= indent && /^\s*\)[,;]?\s*$/.test(l)) break;
      end++;
    }
    const after = lines.slice(i, end + 1).join("\n");
    const before = lines.slice(Math.max(0, i - 4), i).join("\n");
    if (/underline:/.test(after) || /DropdownButtonHideUnderline/.test(before)) return;
    bad.push(`${f.replace(root, "")}:${i + 1} DropdownButton 没去掉默认下划线`);
  });
}

// ── 2. 主题里的全局关闭不许被删 ──
const theme = readFileSync(join(root, "flutter/lib/src/shell/theme.dart"), "utf8");
if (!/inputDecorationTheme:[\s\S]*?enabledBorder:\s*_inputBorder/.test(theme)) {
  bad.push("flutter/lib/src/shell/theme.dart 少了 inputDecorationTheme 的显式 border 套件 —— TextField 会重新长出下划线");
}

if (bad.length > 0) {
  console.error("[flutter-underline] 发现 Material 默认下划线：");
  for (const b of bad) console.error(`  ${b}`);
  process.exit(1);
}
console.log(`[flutter-underline] ok（扫了 ${files.length} 个含 DropdownButton 的文件）`);

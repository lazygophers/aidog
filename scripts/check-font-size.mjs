#!/usr/bin/env node
// 字号下限门禁（`yarn check:font-size`）：全前端文字字号 ≥14px（用户 2026-11-11 拍板「最小字号不能小于14」）。
//
// 扫 src/ 的 .ts/.tsx/.css（排除 themes/、**/generated/、*.test.*），两类违规：
//   ① Tailwind 字号类：`text-xs`（12px）与任意值 <14px（text-[10px] 等）→ 改 text-sm（14px）。
//      允许集：text-sm / text-base / text-lg / text-xl 及 ≥14px 的任意值（text-2xl 等更大类不在此列亦不报）。
//   ② 内联 fontSize / CSS font-size 数值 <14（px 缺省；rem ×16 换算；% 与 em 不在数值形态内，扫不到）。
// 白名单：无（字号一律 ≥14，无装饰性豁免——用户规则无例外）。
// 退出码非 0 = 有 <14px 字号，CI 与本地 `make lint` 均按此判定。

import { readFileSync, readdirSync } from "node:fs";
import { join, relative } from "node:path";

const ROOT = new URL("..", import.meta.url).pathname;
const SRC = join(ROOT, "src");
const MIN_PX = 14;

const EXCLUDED_DIRS = new Set(["node_modules", "dist", "generated"]);

function walk(dir, out) {
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    if (e.isDirectory()) {
      if (EXCLUDED_DIRS.has(e.name) || e.name.startsWith(".")) continue;
      walk(join(dir, e.name), out);
    } else if (/\.(css|ts|tsx)$/.test(e.name) && !e.name.includes(".test.")) {
      out.push(join(dir, e.name));
    }
  }
  return out;
}

// Tailwind 字号类：text-xs / text-[10px] 等（text-sm 及以上放行）
const TW_BAD = /(?<![\w-])text-(xs|\[([0-9.]+)(px|rem)\])/g;
// 内联 fontSize / CSS font-size：fontSize: 12 / fontSize={12} / fontSize: "12px" / font-size: 12px
const FS = /(fontSize|font-size)["']?\s*[:=]\s*[{(]?["']?([0-9.]+)\s*(px|rem)?/gi;

const toPx = (v, unit) => (unit === "rem" ? parseFloat(v) * 16 : parseFloat(v));

const errors = [];
const files = walk(SRC, []);

for (const f of files) {
  if (f.startsWith(join(SRC, "themes") + "/")) continue; // themes 是 token 定义侧
  const lines = readFileSync(f, "utf8").split("\n");
  lines.forEach((line, i) => {
    const rel = `${relative(ROOT, f)}:${i + 1}`;
    for (const m of line.matchAll(TW_BAD)) {
      const v = m[2] !== undefined ? toPx(m[2], m[3]) : 12; // text-xs = 12px
      if (v < MIN_PX)
        errors.push(`${rel} Tailwind 字号类 \`${m[0]}\` < ${MIN_PX}px（改用 text-sm 或更大）`);
    }
    for (const m of line.matchAll(FS)) {
      if (toPx(m[2], m[3]) < MIN_PX)
        errors.push(`${rel} ${m[1]} ${m[2]}${m[3] || "px"} < ${MIN_PX}px`);
    }
  });
}

if (errors.length) {
  console.error(`check-font-size: ${errors.length} 处字号 < ${MIN_PX}px：`);
  for (const e of errors) console.error(`  ${e}`);
  process.exit(1);
}
console.log(`check-font-size: ok（${files.length} 个文件，全部文字字号 ≥ ${MIN_PX}px）`);

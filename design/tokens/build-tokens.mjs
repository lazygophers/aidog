#!/usr/bin/env node
// tokens.json → tokens.css（原型 / Flutter 侧同用）+ lib/tokens.dart（Flutter 版，pub 包 aidog_tokens）
//              + src/themes/tokens.generated.ts（现仓 React 版，mono 主题的色值来源）
// 跑法：node design/tokens/build-tokens.mjs   （无依赖，Node ≥ 18）
// 自检：node design/tokens/build-tokens.mjs --check
//   ① 每个颜色 token 两个 mode 一一对应；② 三份生成物与 tokens.json 当前内容逐字节一致。
//   ② 就是 lint 门禁：改了 tokens.json 不重新生成 → 退出码 1。
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, "..", "..");
const T = JSON.parse(readFileSync(join(here, "tokens.json"), "utf8"));
const drop = (o) => Object.fromEntries(Object.entries(o).filter(([k]) => !k.startsWith("$")));

const colors = drop(T.color);
const check = [];
for (const [k, v] of Object.entries(colors)) {
  if (!("dark" in v) || !("light" in v)) check.push(`color.${k} 缺 dark 或 light`);
}
if (check.length) { console.error(check.join("\n")); process.exit(1); }

// ── CSS ──
const cssVars = (mode) => Object.entries(colors).map(([k, v]) => `  --${k}: ${v[mode]};`).join("\n");
const scale = (name, obj) => Object.entries(drop(obj)).map(([k, v]) =>
  `  --${name}-${k}: ${typeof v === "number" ? v + "px" : v};`).join("\n");
const typeVars = Object.entries(drop(T.type)).filter(([, v]) => typeof v === "object").map(([k, v]) =>
  `  --font-${k}: ${v.weight} ${v.size}px/1.35 var(${v.mono ? "--family-mono" : "--family-sans"});\n` +
  `  --tracking-${k}: ${v.tracking}em;`).join("\n");

const out = {};
out["design/tokens/tokens.css"] = `/* 由 build-tokens.mjs 从 tokens.json 生成，禁止手改 */
:root {
  --family-sans: ${T.type["family-sans"]};
  --family-mono: ${T.type["family-mono"]};
${cssVars("dark")}
${scale("space", T.space)}
${scale("radius", T.radius)}
${scale("layout", T.layout)}
${typeVars}
${Object.entries(T.motion).map(([k, v]) => `  --motion-${k}: ${Array.isArray(v) ? `cubic-bezier(${v.join(",")})` : v + "ms"};`).join("\n")}
}
[data-theme="light"] {
${cssVars("light")}
}
`;

// ── Dart ──
const dartColor = (raw) => {
  const s = String(raw).trim();
  let m = /^#([0-9a-f]{6})$/i.exec(s);
  if (m) return `Color(0xFF${m[1].toUpperCase()})`;
  m = /^rgba\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)\s*\)$/i.exec(s);
  if (m) {
    const a = Math.round(parseFloat(m[4]) * 255).toString(16).padStart(2, "0").toUpperCase();
    const hex = [m[1], m[2], m[3]].map((n) => (+n).toString(16).padStart(2, "0")).join("").toUpperCase();
    return `Color(0x${a}${hex})`;
  }
  return null; // 阴影 / none 之类，走 raw 字符串
};
const dartField = (k) => k.replace(/-(\w)/g, (_, c) => c.toUpperCase());
const colorFields = Object.entries(colors).filter(([, v]) => dartColor(v.dark) && dartColor(v.light));
const rawFields = Object.entries(colors).filter(([, v]) => !(dartColor(v.dark) && dartColor(v.light)));

// 落在 lib/ 下是为了让 design/tokens 成为一个可被 path 依赖的 pub 包
// （flutter/pubspec.yaml 的 aidog_tokens），这样 Flutter 侧不用复制第二份色值。
out["design/tokens/lib/tokens.dart"] = `// 由 build-tokens.mjs 从 tokens.json 生成，禁止手改。
// 用法：AidogColors.dark / AidogColors.light 喂进 ThemeExtension，
// 尺寸与字阶走 AidogSpace / AidogRadius / AidogLayout / AidogType / AidogMotion（与模式无关）。
import 'package:flutter/widgets.dart';

@immutable
class AidogColors {
${colorFields.map(([k]) => `  final Color ${dartField(k)};`).join("\n")}
${rawFields.map(([k]) => `  final String ${dartField(k)}; // 阴影/none，按平台自行解析`).join("\n")}

  const AidogColors({
${[...colorFields, ...rawFields].map(([k]) => `    required this.${dartField(k)},`).join("\n")}
  });

  static const dark = AidogColors(
${colorFields.map(([k, v]) => `    ${dartField(k)}: ${dartColor(v.dark)},`).join("\n")}
${rawFields.map(([k, v]) => `    ${dartField(k)}: ${JSON.stringify(v.dark)},`).join("\n")}
  );

  static const light = AidogColors(
${colorFields.map(([k, v]) => `    ${dartField(k)}: ${dartColor(v.light)},`).join("\n")}
${rawFields.map(([k, v]) => `    ${dartField(k)}: ${JSON.stringify(v.light)},`).join("\n")}
  );
}

class AidogSpace {
${Object.entries(drop(T.space)).map(([k, v]) => `  static const double s${k.replace(/^(\d)/, "_$1")} = ${v.toFixed(1)};`).join("\n")}
}

class AidogRadius {
${Object.entries(drop(T.radius)).map(([k, v]) => `  static const double ${k} = ${v.toFixed(1)};`).join("\n")}
}

class AidogLayout {
${Object.entries(drop(T.layout)).map(([k, v]) => `  static const double ${dartField(k)} = ${v.toFixed(1)};`).join("\n")}
}

class AidogType {
  static const familySans = 'Inter';
  static const familyMono = 'JetBrains Mono';
${Object.entries(drop(T.type)).filter(([, v]) => typeof v === "object").map(([k, v]) =>
  `  static const ${dartField(k)} = TextStyle(fontFamily: ${v.mono ? "familyMono" : "familySans"}, ` +
  `fontSize: ${v.size.toFixed(1)}, fontWeight: FontWeight.w${v.weight}, letterSpacing: ${(v.tracking * v.size).toFixed(2)}, height: 1.35);`).join("\n")}
}

class AidogMotion {
${Object.entries(T.motion).filter(([, v]) => !Array.isArray(v)).map(([k, v]) => `  static const ${dartField(k)} = Duration(milliseconds: ${v});`).join("\n")}
${Object.entries(T.motion).filter(([, v]) => Array.isArray(v)).map(([k, v]) => `  static const ${dartField(k)} = Cubic(${v.map((n) => n.toFixed(2)).join(", ")});`).join("\n")}
}
`;

// ── TS（现仓 React 版）──
// 只出颜色：间距/圆角/字阶属版式，现仓版式不由本表驱动（票 13 边界）。
const tsMode = (mode) => Object.entries(colors)
  .map(([k, v]) => `    ${JSON.stringify(k)}: ${JSON.stringify(v[mode])},`).join("\n");

out["src/themes/tokens.generated.ts"] = `// 由 design/tokens/build-tokens.mjs 从 design/tokens/tokens.json 生成，禁止手改。
// 改色请改 tokens.json 再跑 \`yarn tokens\`；\`yarn check:tokens\` 会拦住忘记重新生成的情况。
export const tokens = {
  dark: {
${tsMode("dark")}
  },
  light: {
${tsMode("light")}
  },
} as const;
`;

if (process.argv.includes("--check")) {
  const stale = Object.entries(out).filter(([rel, body]) => {
    try { return readFileSync(join(repoRoot, rel), "utf8") !== body; } catch { return true; }
  });
  if (stale.length) {
    console.error(`生成物与 tokens.json 不一致：\n${stale.map(([r]) => "  " + r).join("\n")}\n` +
      `跑 \`yarn tokens\` 重新生成后再提交。`);
    process.exit(1);
  }
  console.log(`ok · ${Object.keys(colors).length} color × 2 mode · ${Object.keys(out).length} 份生成物与真值源一致`);
  process.exit(0);
}

for (const [rel, body] of Object.entries(out)) {
  const abs = join(repoRoot, rel);
  mkdirSync(dirname(abs), { recursive: true });
  writeFileSync(abs, body);
}
console.log(`写出 ${Object.keys(out).join(" + ")} · ${Object.keys(colors).length} 个颜色 token × 2 模式`);

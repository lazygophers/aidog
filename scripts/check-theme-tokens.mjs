#!/usr/bin/env node
// 主题 token 引用完整性门禁（`yarn check:theme-tokens`）。
//
// 背景：2026-11-11 路由画布 .rc-node 写了 `background: var(--surface)`，但全项目从未定义过
// `--surface`（真名是 --bg-surface 别名 → --card）。无效变量让 background 解析失败，节点两态
// 都没有底色。静态扫描器（tsc / vite / check:tokens）都拦不住 CSS 无效变量，本脚本补这个缺口：
//
//   1. 收集「定义集合」：src/ 下全部 .css 的 `--x:` 声明 + src/themes/*.ts 的 "--x" 键。
//   2. 扫「引用」：src/ 的 .ts/.tsx/.css 里的 var(--x)（排除 themes/ 与 generated/ ——
//      themes 是定义侧不是消费侧）。
//   3. 引用 ∉ 定义集合 ∪ 动态白名单 → 报错 file:line + token 名，exit 1。
//
// 退出码非 0 = 有未知 token 引用，CI 与本地 `make lint` 均按此判定。

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const ROOT = new URL("..", import.meta.url).pathname;
const SRC = join(ROOT, "src");

/** 运行时动态注入、静态扫不出定义的白名单（禁随意扩充；新条目须在此注明来源）：
 *  - `--radix-*`：Radix UI 运行时注入（dropdown-menu.tsx / select.tsx / FieldRenderer.tsx 等）。
 *  - `--color-*`：图表组件 `var(--color-${key})` 模板动态拼名（BarChart / LineChart / colorScale 等）。
 *  - `--font-mono`：Tailwind v4 从 @theme font-mono 构建期发出的官方变量，源码中无定义。 */
const PREFIX_ALLOWLIST = ["--radix-", "--color-"];
const EXACT_ALLOWLIST = ["--font-mono"];

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

const errors = [];

// ── 1. 定义集合 ───────────────────────────────────────────────────────────
const files = walk(SRC, []);
const defined = new Set();
for (const f of files) {
  const text = readFileSync(f, "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
  for (const m of text.matchAll(/(^|[^"\w])(--[\w-]+)\s*:/g)) defined.add(m[2]); // CSS / 内联样式声明
  for (const m of text.matchAll(/"(--[\w-]+)"/g)) defined.add(m[1]); // themes/*.ts 的对象键
}

// ── 2. 引用扫描 ───────────────────────────────────────────────────────────
for (const f of files) {
  const isThemes = f.startsWith(join(SRC, "themes") + "/");
  if (isThemes) continue; // themes 是定义侧，其 var() 均指向自身定义的键
  const text = readFileSync(f, "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
  const lines = text.split("\n");
  lines.forEach((line, i) => {
    for (const m of line.matchAll(/var\(\s*(--[\w-]+)/g)) {
      const token = m[1];
      if (EXACT_ALLOWLIST.includes(token)) continue;
      if (PREFIX_ALLOWLIST.some((p) => token.startsWith(p))) continue;
      if (!defined.has(token)) {
        errors.push(
          `${relative(ROOT, f)}:${i + 1} 引用了未定义的 CSS token \`${token}\`。` +
            `请改用已定义的 token（查 src/styles/globals.css / src/themes/mono.ts），` +
            `确需运行时注入的加进本脚本顶部白名单并注明来源。`,
        );
      }
    }
  });
}

if (errors.length) {
  console.error(`check-theme-tokens: ${errors.length} 处未知 token 引用：`);
  for (const e of errors) console.error(`  ${e}`);
  process.exit(1);
}
console.log(`check-theme-tokens: ok（${files.length} 个文件，定义 ${defined.size} 个 token，全部引用可解析）`);

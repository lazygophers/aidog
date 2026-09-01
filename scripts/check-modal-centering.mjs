#!/usr/bin/env node
// 弹窗居中回归护栏（`yarn check:modal`）。
//
// 背景：Tailwind v4 把 .fixed/.absolute 放在 `@layer utilities`，而 CSS Cascade Layers 规定
// **layer 外（unlayered）的声明压过任何 layer 内的声明**，与特异性无关。所以 globals.css 里
// 裸写一条 `.glass-surface { position: relative }`，就能把挂了该 class 的 Radix DialogContent
// 从 `position: fixed` 打回文档流 —— 表现是「弹窗不按窗口居中，跟着页面走」。
//
// 本脚本守三条：
//   A. globals.css 的 unlayered 区不得声明 position（白名单：弹窗定位硬保障那几个 .ui-* 类）。
//   B. shadcn Dialog / AlertDialog / Sheet 的 Overlay 与 Content 必须挂 .ui-dialog-* 保障类。
//   C. 手写弹窗（position:fixed + inset:0 的遮罩）必须经 createPortal 挂 document.body。
//
// 退出码非 0 = 有违规，CI 与本地 `yarn check:modal` 均按此判定。

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const ROOT = new URL("..", import.meta.url).pathname;
const CSS = join(ROOT, "src/styles/globals.css");
const SRC = join(ROOT, "src");

/** 允许 unlayered 声明 position 的选择器（弹窗定位硬保障，见 globals.css 末尾）。 */
const POSITION_ALLOWLIST = [".ui-dialog-overlay", ".ui-dialog-panel", ".ui-fixed-panel"];

const errors = [];

// ── A. globals.css unlayered 区不得出现 position ──────────────────────────
{
  const css = readFileSync(CSS, "utf8");
  // 逐字符扫，记录当前是否在 @layer {...} 块内（层内允许 position）。
  let depth = 0;
  let layerDepth = null; // 进入 @layer 块时的 depth
  let selector = "";
  let line = 1;
  let inRule = false;
  let buf = "";
  for (let i = 0; i < css.length; i++) {
    const ch = css[i];
    if (ch === "\n") line++;
    if (css.startsWith("/*", i)) {
      const end = css.indexOf("*/", i + 2);
      const skipped = css.slice(i, end < 0 ? css.length : end + 2);
      line += (skipped.match(/\n/g) || []).length;
      i = end < 0 ? css.length : end + 1;
      continue;
    }
    if (ch === "{") {
      const head = buf.trim();
      buf = "";
      depth++;
      if (head.startsWith("@layer") && layerDepth === null) layerDepth = depth;
      else if (!head.startsWith("@")) {
        inRule = layerDepth === null; // unlayered 规则才检查
        selector = head;
      }
      continue;
    }
    if (ch === "}") {
      if (layerDepth === depth) layerDepth = null;
      depth--;
      inRule = false;
      buf = "";
      continue;
    }
    if (ch === ";") {
      const decl = buf.trim();
      buf = "";
      // 伪元素（::before/::after）豁免：它们不是元素本身，调用方的 className 落不到伪元素上，
      // 压不到 Radix Content 的 .fixed。
      if (inRule && /^position\s*:/.test(decl) && !selector.includes("::")) {
        const allowed = POSITION_ALLOWLIST.some((s) => selector.includes(s));
        if (!allowed) {
          errors.push(
            `globals.css:${line} unlayered 规则 \`${selector}\` 声明了 \`${decl}\`。` +
              `unlayered 压过 Tailwind utilities 的 .fixed/.absolute，会让挂该 class 的弹窗掉出视口居中。` +
              `请把 position 移进 \`@layer components\`（见 globals.css 末尾「定位上下文」块）。`,
          );
        }
      }
      continue;
    }
    buf += ch;
  }
}

// ── B. shadcn 弹窗组件必须挂保障类 ────────────────────────────────────────
const UI_GUARDS = [
  ["src/components/ui/dialog.tsx", ["ui-dialog-overlay", "ui-dialog-panel"]],
  ["src/components/ui/alert-dialog.tsx", ["ui-dialog-overlay", "ui-dialog-panel"]],
  ["src/components/ui/sheet.tsx", ["ui-dialog-overlay", "ui-fixed-panel"]],
];
for (const [rel, classes] of UI_GUARDS) {
  const src = readFileSync(join(ROOT, rel), "utf8");
  for (const cls of classes) {
    if (!src.includes(cls)) {
      errors.push(
        `${rel} 缺少弹窗定位保障类 \`${cls}\`。Overlay / Content 的 className 必须带它，` +
          `否则调用方传入的自定义 class 能把 position 压回 relative。`,
      );
    }
  }
}

// ── C. 手写遮罩必须 createPortal ─────────────────────────────────────────
function walk(dir) {
  const out = [];
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    const st = statSync(p);
    if (st.isDirectory()) out.push(...walk(p));
    else if (p.endsWith(".tsx") && !p.endsWith(".test.tsx")) out.push(p);
  }
  return out;
}
for (const file of walk(SRC)) {
  if (file.includes("/components/ui/")) continue; // shadcn 原件由 B 覆盖
  const src = readFileSync(file, "utf8");
  // 居中弹窗特征：同一 style 对象里 position:"fixed" + inset:0 + 居中布局。
  // 只有 inset:0 没有居中（点外部关闭用的透明捕获层）不算弹窗，豁免。
  const hasCenteredOverlay =
    /position:\s*"fixed"[^}]{0,400}?\binset:\s*0[^}]{0,400}?(alignItems|justifyContent):\s*"center"/s.test(src);
  if (hasCenteredOverlay && !src.includes("createPortal")) {
    errors.push(
      `${relative(ROOT, file)} 手写了 position:fixed + inset:0 的全屏遮罩，但没有 createPortal。` +
        `祖先若有 transform / backdrop-filter，fixed 会退化为相对该祖先定位，弹窗只在页面内居中。` +
        `改用 @/components/ui/dialog 的 <Dialog>，或 createPortal(…, document.body)。`,
    );
  }
}

if (errors.length) {
  console.error(`\n❌ 弹窗居中检查未通过（${errors.length} 项）：\n`);
  for (const e of errors) console.error(`  • ${e}\n`);
  process.exit(1);
}
console.log("✅ 弹窗居中检查通过（unlayered position / 保障类 / 手写遮罩 portal）");

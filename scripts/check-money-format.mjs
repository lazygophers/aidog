#!/usr/bin/env node
// 金额格式化门禁（`yarn check:money`）：src/ 非测试 ts/tsx 不得手工拼接金额。
// 费用显示一律走 utils/formatters.ts 的 formatMoney / formatAmount，否则切显示币种后漏换算。
// 命中即 exit 1；确属非费用 / 真值源的在 ALLOWLIST 登记（每项写原因）。

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = new URL("..", import.meta.url).pathname;

/** 违规模式：字符串字面量 "$" / '$' 参与拼接，或模板里 `$${`，或 replace("$", …)。 */
export const PATTERNS = [
  { re: /["']\$["']\s*\+|\+\s*["']\$["']/, why: '"$" 字符串拼接' },
  { re: /`\$\$\{/, why: "模板字符串 `$${…}`" },
  { re: /\?\?\s*["']\$["']|\?\s*["']\$["']\s*:/, why: '"$" 作符号兜底/三元' },
  { re: /\.replace(All)?\(\s*["']\$["']/, why: '.replace("$", …)' },
];

/** 豁免（整文件；格式化真值源 formatters/displayCurrency 当前零命中，无需登记）：file 为相对仓库根的路径，why 写原因。 */
export const ALLOWLIST = [
  { file: "src/components/settings/statusline-segments.ts", why: "外部 statusline 脚本前缀，输出保持 USD（spec Out of Scope）" },
  { file: "src/pages/platforms/formSections.tsx", why: "限流窗口 usd 单位标签 / 预算输入框 $ 前缀：输入侧保持 USD 语义（story 22）" },
];

function walk(dir, out) {
  for (const n of readdirSync(dir)) {
    const p = join(dir, n);
    if (statSync(p).isDirectory()) walk(p, out);
    else if (/\.tsx?$/.test(n) && !/\.(test|spec)\.tsx?$/.test(n) && !n.endsWith(".d.ts")) out.push(p);
  }
  return out;
}

/** 扫 files（绝对路径），返回 [{file, line, why, text}]；root 用于算相对路径匹配白名单。 */
export function scan(files, root = ROOT, allow = ALLOWLIST) {
  const allowed = new Set(allow.map((a) => a.file));
  const hits = [];
  for (const f of files) {
    const rel = relative(root, f);
    if (allowed.has(rel)) continue;
    readFileSync(f, "utf8").split("\n").forEach((text, i) => {
      if (/^\s*(\/\/|\*|\/\*)/.test(text)) return; // 纯注释行
      for (const { re, why } of PATTERNS) {
        if (re.test(text)) hits.push({ file: rel, line: i + 1, why, text: text.trim() });
      }
    });
  }
  return hits;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const hits = scan(walk(join(ROOT, "src"), []));
  for (const h of hits) console.error(`${h.file}:${h.line}: ${h.why}\n    ${h.text}`);
  if (hits.length) {
    console.error(`\ncheck:money 失败：${hits.length} 处手工拼金额。改走 formatMoney/formatAmount，或在 ALLOWLIST 登记原因。`);
    process.exit(1);
  }
  console.log("check:money OK");
}

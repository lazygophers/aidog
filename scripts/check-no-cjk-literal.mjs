#!/usr/bin/env node
// 中文字面量门禁（`yarn check:no-cjk-literal`）：src/ 非测试 ts/tsx 的字符串 / 模板字面量 / JSX 文本不得含 CJK 字符。
// 用户可见文案一律走 i18n（曾有 formatRelativeTime 硬编码「N 小时前」，英文界面显示中文）。注释不查。
// 命中即 exit 1，全量输出 file:line；确需保留的在 ALLOWLIST 登记（file + reason，可选 match 只豁免含该串的行）。

import { readFileSync, readdirSync, statSync } from "node:fs";
import { createRequire } from "node:module";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = new URL("..", import.meta.url).pathname;
// shortcut: 根目录 typescript 是 7.x（Go 版，无 createSourceFile），借 docs workspace 声明的 typescript 6；根包换回带 JS API 的版本后改为直接 import。
const ts = createRequire(join(ROOT, "docs/package.json"))("typescript");

const CJK = /[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}\p{Script=Hangul}]/u;
const LITERAL_KINDS = new Set([
  ts.SyntaxKind.StringLiteral,
  ts.SyntaxKind.NoSubstitutionTemplateLiteral,
  ts.SyntaxKind.TemplateHead,
  ts.SyntaxKind.TemplateMiddle,
  ts.SyntaxKind.TemplateTail,
  ts.SyntaxKind.JsxText,
]);

/** 豁免：file 为相对仓库根路径；match 缺省 = 整文件，否则只豁免源码行含 match 的命中；reason 必填。 */
export const ALLOWLIST = [
  // 语言自称名：语言选项按各语言本身的写法显示，不随界面语言翻译。
  { file: "src/services/claude-settings-schema.ts", match: 'family: "中文"', reason: "语言自称名（中文语族名）" },
  { file: "src/services/claude-settings-schema.ts", match: '"简体（', reason: "语言自称名（简体中文变体）" },
  { file: "src/services/claude-settings-schema.ts", match: '"繁体（', reason: "语言自称名（繁体中文变体）" },
  { file: "src/services/claude-settings-schema.ts", match: '"日本語"', reason: "语言自称名（日语）" },
  { file: "src/services/claude-settings-schema.ts", match: '"한국어"', reason: "语言自称名（韩语）" },
  // 与后端逐字一致的内部名：改了就对不上后端。
  { file: "src/components/settings/CodingToolsSettings.tsx", match: '"内置·日期格式改写防检测"', reason: "内置规则名，须与 Rust schema.rs builtin_rule_specs 逐字一致（按名查 middleware_rule）" },
  { file: "src/pages/Groups/useGroupData.ts", match: '"未匹配"', reason: "后端 fallback 直通落库的虚拟桶 group_key，按此键取统计" },
  { file: "src/components/settings/NotificationEventList.tsx", match: 'defaultTemplate: "', reason: "逐字镜像后端 notification.rs default_template_for_event；后端发送的默认通知文案即此中文，占位符须与之一致" },
  { file: "src/components/settings/statusline-segments.ts", match: '"余额 "', reason: "前缀默认值镜像 Rust statusline.rs DEFAULT_OPTIONS，写进状态行脚本输出" },
  { file: "src/components/settings/statusline-segments.ts", match: '"折算$"', reason: "前缀默认值镜像 Rust statusline.rs DEFAULT_OPTIONS，写进状态行脚本输出" },
  { file: "src/components/settings/statusline-segments.ts", match: '"缓存 "', reason: "前缀默认值镜像 Rust statusline.rs DEFAULT_OPTIONS / 默认段，写进状态行脚本输出" },
  { file: "src/pages/TrayConfigTab.tsx", match: '["天", "小时", "分"]', reason: "托盘倒计时 cn 格式，与 Rust tray_render.rs format_countdown 逐字一致（托盘标签无 i18n）" },
  { file: "src/pages/TrayConfigTab.tsx", match: "${d}天 ", reason: "托盘倒计时 clock 格式天数前缀，与 Rust tray_render.rs format_countdown 逐字一致" },
  { file: "src/pages/TrayConfigTab.tsx", match: '"峰" : "平"', reason: "托盘高峰段取值，与 Rust tray_render.rs peak_item_parts 逐字一致（托盘标签无 i18n）" },
];

/** 列出 dir 下待查文件（绝对路径）：排除测试、.d.ts、src/services/api/types/generated/。 */
export function collect(dir, root = ROOT, out = []) {
  for (const n of readdirSync(dir)) {
    const p = join(dir, n);
    if (statSync(p).isDirectory()) {
      if (relative(root, p) !== join("src", "services", "api", "types", "generated")) collect(p, root, out);
    } else if (/\.tsx?$/.test(n) && !/\.(test|spec)\.tsx?$/.test(n) && !n.endsWith(".d.ts")) out.push(p);
  }
  return out;
}

/** 扫 files（绝对路径），返回 [{file, line, text}]。 */
export function scan(files, root = ROOT, allow = ALLOWLIST) {
  const hits = [];
  for (const f of files) {
    const rel = relative(root, f);
    const rules = allow.filter((a) => a.file === rel);
    if (rules.some((a) => !a.match)) continue;
    const src = readFileSync(f, "utf8");
    const lines = src.split("\n");
    const sf = ts.createSourceFile(f, src, ts.ScriptTarget.Latest, true, f.endsWith(".tsx") ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
    const visit = (node) => {
      if (LITERAL_KINDS.has(node.kind)) {
        const start = node.getStart(sf);
        const m = CJK.exec(node.getText(sf));
        if (m) {
          const line = sf.getLineAndCharacterOfPosition(start + m.index).line;
          const text = lines[line];
          if (!rules.some((a) => text.includes(a.match))) hits.push({ file: rel, line: line + 1, text: text.trim().slice(0, 160) });
        }
      }
      ts.forEachChild(node, visit);
    };
    visit(sf);
  }
  return hits;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const hits = scan(collect(join(ROOT, "src")));
  for (const h of hits) console.error(`${h.file}:${h.line}: ${h.text}`);
  if (hits.length) {
    console.error(`\ncheck:no-cjk-literal 失败：${hits.length} 处 CJK 字面量。改走 i18n，或在 ALLOWLIST 登记原因。`);
    process.exit(1);
  }
  console.log("check:no-cjk-literal OK");
}

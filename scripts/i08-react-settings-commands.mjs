// 解析出「设置这一批页面在 React 版实际调用的全部命令名」，写成清单给 Flutter 侧
// 的零差集测试当真值源（`flutter/test/settings/command_coverage_test.dart`）。
//
// 为什么要脚本而不是手列：这批是 13 个子页 + 30 多个组件，命令散在
// `src/services/api/` 的命名空间对象里（`platformApi.list()` 这种），手列必漏。
// 漏一条 = Flutter 少一个功能而测试还是绿的，正是本票要防的东西。
//
// 解析口径：
//   1. 文件内直接 `invoke("cmd")`；
//   2. 从 `services/api` 具名导入的符号 —— 函数取整段，命名空间对象**只取本文件
//      真正访问过的成员**（否则 import 一个 platformApi 就把它全部方法算进来）。
//
// 用法：
//   node scripts/i08-react-settings-commands.mjs           # 写清单
//   node scripts/i08-react-settings-commands.mjs --check    # 只比对，不一致 exit 1
//   node scripts/i08-react-settings-commands.mjs --report   # 打印逐文件明细

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const OUT = path.join(root, 'flutter', 'test', 'settings', 'react_settings_commands.txt');

const r = (p) => path.join(root, p);
const dirFiles = (d, re = /\.tsx?$/) =>
  fs
    .readdirSync(r(d))
    .filter((f) => re.test(f) && !f.includes('.test.'))
    .map((f) => `${d}/${f}`);

/** 本票范围：`.scratch/flutter-frontend/impl/I08-pages-c.md` 点名的那批。 */
const FILES = [
  'src/pages/Settings.tsx',
  'src/pages/AppSettings.tsx',
  'src/pages/CodexSettings.tsx',
  'src/pages/PiSettings.tsx',
  'src/pages/TrayConfigTab.tsx',
  'src/pages/PopoverConfigTab.tsx',
  ...dirFiles('src/components/settings', /\.tsx$/),
  ...dirFiles('src/pages/AppSettings'),
  ...dirFiles('src/pages/PopoverConfigTab'),
  ...dirFiles('src/components/settings/ImportExport'),
];

const apiSrc = {};
(function walk(d) {
  for (const e of fs.readdirSync(d, { withFileTypes: true })) {
    const p = path.join(d, e.name);
    if (e.isDirectory()) walk(p);
    else if (e.name.endsWith('.ts')) apiSrc[p] = fs.readFileSync(p, 'utf8');
  }
})(r('src/services/api'));
const API = Object.entries(apiSrc);

/** `invoke<...>("cmd"` → cmd。泛型里可能有嵌套尖括号与分号，所以只排掉括号。 */
function cmdsIn(text) {
  const s = new Set();
  const re = /\binvoke\s*(?:<[^()]{0,300}?>)?\s*\(\s*["'`]([a-z_0-9]+)["'`]/g;
  for (const m of text.matchAll(re)) s.add(m[1]);
  return s;
}

/** 取 `export const SYM = ...` / `export function SYM` 到下一个顶层 export 之间的整段。 */
function blockFor(sym) {
  for (const [, src] of API) {
    const i = src.search(new RegExp(`export\\s+(?:const|async function|function)\\s+${sym}\\b`));
    if (i < 0) continue;
    const rest = src.slice(i + 10);
    const nx = rest.search(/\nexport\s/);
    return src.slice(i, nx < 0 ? src.length : i + 10 + nx);
  }
  return null;
}

/** 在对象字面量块里切出某个成员的那一段（到下一个同缩进成员为止）。 */
function memberBlock(block, member) {
  const i = block.search(new RegExp(`(?:^|[\\s,{])(?:async\\s+)?${member}\\s*[(:]`, 'm'));
  if (i < 0) return null;
  const rest = block.slice(i + 1);
  const nx = rest.search(/\n {2}(?:async )?[a-zA-Z_][a-zA-Z_0-9]*\s*[(:]/);
  return block.slice(i, nx < 0 ? block.length : i + 1 + nx);
}

const perFile = {};
const union = new Set();
const notes = [];

for (const f of FILES) {
  const src = fs.readFileSync(r(f), 'utf8');
  const cmds = new Set(cmdsIn(src));
  for (const m of src.matchAll(/import\s*\{([^}]*)\}\s*from\s*["'][^"']*services\/api[^"']*["']/g)) {
    for (const raw of m[1].split(',')) {
      if (/^\s*type\s/.test(raw)) continue;
      const sym = raw.trim().replace(/^type\s+/, '').split(/\s+as\s+/)[0].trim();
      if (!sym) continue;
      const b = blockFor(sym);
      if (!b) {
        if (!/^[A-Z]/.test(sym)) notes.push(`${f}: 解析不到 ${sym}`);
        continue;
      }
      const isObj = /^export\s+const\s+\w+\s*(?::[^=]*)?=\s*\{/.test(b);
      if (!isObj) {
        for (const c of cmdsIn(b)) cmds.add(c);
        continue;
      }
      const used = new Set(
        [...src.matchAll(new RegExp(`\\b${sym}\\s*\\.\\s*([a-zA-Z_0-9]+)`, 'g'))].map((x) => x[1]),
      );
      if (used.size === 0) continue; // 只作类型用
      for (const mem of used) {
        const mb = memberBlock(b, mem);
        if (!mb) {
          notes.push(`${f}: 切不出 ${sym}.${mem}`);
          continue;
        }
        const c = cmdsIn(mb);
        if (c.size === 0) notes.push(`${f}: ${sym}.${mem} 里没有 invoke`);
        for (const x of c) cmds.add(x);
      }
    }
  }
  perFile[f] = [...cmds].sort();
  for (const c of cmds) union.add(c);
}

if (notes.length > 0) {
  console.error('[i08-commands] 有解析不掉的符号，清单可能不全：');
  for (const n of notes) console.error('  ' + n);
  process.exit(2);
}

if (process.argv.includes('--report')) {
  for (const [f, c] of Object.entries(perFile)) {
    console.log(`${f}\n  ${c.join(' ') || '(无)'}`);
  }
}

const sorted = [...union].sort();
const text =
  '# React 版「设置」13 子页实际调用的命令（由 scripts/i08-react-settings-commands.mjs 生成，禁手改）\n' +
  `# 共 ${sorted.length} 条\n` +
  sorted.join('\n') +
  '\n';

if (process.argv.includes('--check')) {
  const cur = fs.existsSync(OUT) ? fs.readFileSync(OUT, 'utf8') : '';
  if (cur !== text) {
    console.error(
      `[i08-commands] ${path.relative(root, OUT)} 与 src/ 现状不一致。` +
        '\n  跑 `node scripts/i08-react-settings-commands.mjs` 重新生成。',
    );
    process.exit(1);
  }
  console.log(`[i08-commands] ok（${sorted.length} 条）`);
} else {
  fs.mkdirSync(path.dirname(OUT), { recursive: true });
  fs.writeFileSync(OUT, text);
  console.log(`[i08-commands] ${path.relative(root, OUT)}：${sorted.length} 条`);
}

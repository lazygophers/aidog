// 「某一批 React 页面实际调用了哪些后端命令」的解析器。
//
// 票 I08 先写了这段解析（`scripts/i08-react-settings-commands.mjs`），票 I07 的平台 /
// 分组 / 日志批次要用同一口径，故抽到这里共用 —— 两批各自只剩一张文件清单。
// 行为与 I08 落地时逐字一致（`--check` 对既有清单仍字节相等）。
//
// 解析口径：
//   1. 文件内直接 `invoke("cmd")`；
//   2. 从 `services/api` 具名导入的符号 —— 函数取整段，命名空间对象**只取本文件
//      真正访问过的成员**（否则 import 一个 platformApi 就把它全部方法算进来）。

import fs from 'node:fs';
import path from 'node:path';

/** 读 `src/services/api/**\/*.ts` 全文，供符号定位。 */
export function loadApiSources(root) {
  const apiSrc = {};
  (function walk(d) {
    for (const e of fs.readdirSync(d, { withFileTypes: true })) {
      const p = path.join(d, e.name);
      if (e.isDirectory()) walk(p);
      else if (e.name.endsWith('.ts')) apiSrc[p] = fs.readFileSync(p, 'utf8');
    }
  })(path.join(root, 'src/services/api'));
  return Object.entries(apiSrc);
}

/** `invoke<...>("cmd"` → cmd。泛型里可能有嵌套尖括号与分号，所以只排掉括号。 */
export function cmdsIn(text) {
  const s = new Set();
  const re = /\binvoke\s*(?:<[^()]{0,300}?>)?\s*\(\s*["'`]([a-z_0-9]+)["'`]/g;
  for (const m of text.matchAll(re)) s.add(m[1]);
  return s;
}

/** 取 `export const SYM = ...` / `export function SYM` 到下一个顶层 export 之间的整段。 */
export function blockFor(API, sym) {
  for (const [, src] of API) {
    const i = src.search(
      new RegExp(`export\\s+(?:const|async function|function)\\s+${sym}\\b`),
    );
    if (i < 0) continue;
    const rest = src.slice(i + 10);
    const nx = rest.search(/\nexport\s/);
    return src.slice(i, nx < 0 ? src.length : i + 10 + nx);
  }
  return null;
}

/** 在对象字面量块里切出某个成员的那一段（到下一个同缩进成员为止）。 */
export function memberBlock(block, member) {
  const i = block.search(
    new RegExp(`(?:^|[\\s,{])(?:async\\s+)?${member}\\s*[(:]`, 'm'),
  );
  if (i < 0) return null;
  const rest = block.slice(i + 1);
  const nx = rest.search(/\n {2}(?:async )?[a-zA-Z_][a-zA-Z_0-9]*\s*[(:]/);
  return block.slice(i, nx < 0 ? block.length : i + 1 + nx);
}

/** 扫一批文件，返回 `{ perFile, union, notes }`。notes 非空 = 有符号没解析掉，清单不可信。 */
export function scanFiles(root, files) {
  const API = loadApiSources(root);
  const perFile = {};
  const union = new Set();
  const notes = [];

  for (const f of files) {
    const src = fs.readFileSync(path.join(root, f), 'utf8');
    const cmds = new Set(cmdsIn(src));
    for (const m of src.matchAll(
      /import\s*\{([^}]*)\}\s*from\s*["'][^"']*services\/api[^"']*["']/g,
    )) {
      for (const raw of m[1].split(',')) {
        if (/^\s*type\s/.test(raw)) continue;
        const sym = raw.trim().replace(/^type\s+/, '').split(/\s+as\s+/)[0].trim();
        if (!sym) continue;
        const b = blockFor(API, sym);
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
          [...src.matchAll(new RegExp(`\\b${sym}\\s*\\.\\s*([a-zA-Z_0-9]+)`, 'g'))].map(
            (x) => x[1],
          ),
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

  return { perFile, union, notes };
}

/** 目录里的 .ts/.tsx（排掉测试文件），返回相对 root 的路径。 */
export function dirFiles(root, d, re = /\.tsx?$/) {
  return fs
    .readdirSync(path.join(root, d))
    .filter((f) => re.test(f) && !f.includes('.test.'))
    .map((f) => `${d}/${f}`);
}

/**
 * 统一的 CLI 尾巴：`--report` 打逐文件明细，`--check` 只比对，缺省写文件。
 * [header] 是清单第一行的说明（不含 `# 共 N 条` 那行，那行本函数补）。
 */
export function emit({ root, out, header, perFile, union, notes, argv }) {
  if (notes.length > 0) {
    console.error('[react-commands] 有解析不掉的符号，清单可能不全：');
    for (const n of notes) console.error('  ' + n);
    process.exit(2);
  }

  if (argv.includes('--report')) {
    for (const [f, c] of Object.entries(perFile)) {
      console.log(`${f}\n  ${c.join(' ') || '(无)'}`);
    }
  }

  const sorted = [...union].sort();
  const text = `${header}\n# 共 ${sorted.length} 条\n${sorted.join('\n')}\n`;

  if (argv.includes('--check')) {
    const cur = fs.existsSync(out) ? fs.readFileSync(out, 'utf8') : '';
    if (cur !== text) {
      console.error(
        `[react-commands] ${path.relative(root, out)} 与 src/ 现状不一致。` +
          '\n  重新跑一次不带 --check 的同一条命令即可重新生成。',
      );
      process.exit(1);
    }
    console.log(`[react-commands] ok（${sorted.length} 条）`);
  } else {
    fs.mkdirSync(path.dirname(out), { recursive: true });
    fs.writeFileSync(out, text);
    console.log(`[react-commands] ${path.relative(root, out)}：${sorted.length} 条`);
  }
  return sorted;
}

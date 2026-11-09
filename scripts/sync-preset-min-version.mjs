// 从官方文档推导 Claude Code 预设的 minimumVersion（预设里所有键所需 CLI 版本的最大值）。
//   node scripts/sync-preset-min-version.mjs [预设路径]          校验：文件值 < 计算值 exit 1
//   node scripts/sync-preset-min-version.mjs --write [预设路径]  写入/更新顶层 minimumVersion
//   node scripts/sync-preset-min-version.mjs --self-test         解析函数自检
// 退出码：0 通过 / 1 值过低 / 2 抓取失败。默认预设 lazygophers.json（其它预设无 minimumVersion，不管）。
// 限制：只看"键存在"所需版本，不看枚举值/规则语法/hook 事件级版本；块内取第一个 Requires（键级，子选项级在后）。
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import assert from 'node:assert/strict';

const DOCS = 'https://code.claude.com/docs/en/';
const REQ = /Requires (?:Claude Code )?v(\d+\.\d+\.\d+)/;
// 键名用户自定义的 map：不展开，env 另查 env-vars 表
const OPAQUE = new Set(['env', 'hooks', 'enabledPlugins', 'extraKnownMarketplaces']);

const cmp = (a, b) => {
  const x = a.split('.').map(Number), y = b.split('.').map(Number);
  return x[0] - y[0] || x[1] - y[1] || x[2] - y[2];
};

// settings-reference：`### \`key\`` 小节 -> 块内第一个 Requires 版本（无则 null）
export function parseSettings(md) {
  const out = new Map();
  let key = null;
  for (const line of md.split('\n')) {
    if (/^#{1,3} /.test(line)) {
      key = (line.match(/^#{3} `([^`]+)`\s*$/) || [])[1] ?? null;
      if (key && !out.has(key)) out.set(key, null);
      continue;
    }
    if (key && out.get(key) === null) {
      const m = line.match(REQ);
      if (m) out.set(key, m[1]);
    }
  }
  return out;
}

// env-vars：表格行 | `VAR` | ... Requires Claude Code vX ... |
export function parseEnv(md) {
  const out = new Map();
  for (const line of md.split('\n')) {
    const k = (line.match(/^\| `([A-Z0-9_]+)` \|/) || [])[1];
    if (k) out.set(k, (line.match(REQ) || [])[1] ?? null);
  }
  return out;
}

export function leafPaths(obj, prefix = '') {
  const out = [];
  for (const [k, v] of Object.entries(obj)) {
    const p = prefix ? `${prefix}.${k}` : k;
    if (v && typeof v === 'object' && !Array.isArray(v) && !OPAQUE.has(p)) out.push(...leafPaths(v, p));
    else out.push(p);
  }
  return out;
}

// 最长前缀匹配文档小节
function lookup(path, map) {
  const parts = path.split('.');
  for (let n = parts.length; n > 0; n--) {
    const k = parts.slice(0, n).join('.');
    if (map.has(k)) return { key: k, ver: map.get(k) };
  }
  return null;
}

function selfTest() {
  const s = parseSettings(
    '## A\n### `foo`\nx. Requires Claude Code v2.1.10 or later. Later Requires Claude Code v2.1.99\n### `bar.baz`\nnone\n#### sub\nRequires v2.0.5\n### `desk`\nClaude Desktop v9.9.9\n',
  );
  assert.equal(s.get('foo'), '2.1.10');
  assert.equal(s.get('bar.baz'), '2.0.5');
  assert.equal(s.get('desk'), null);
  const e = parseEnv('| `A_B` | text Requires Claude Code v2.1.3 or later. |\n| `C` | none |\n');
  assert.equal(e.get('A_B'), '2.1.3');
  assert.equal(e.get('C'), null);
  assert.deepEqual(leafPaths({ a: { b: 1, c: [1] }, env: { X: 1 } }), ['a.b', 'a.c', 'env']);
  assert.equal(lookup('a.b.c', new Map([['a', '1.0.0']])).key, 'a');
  assert.ok(cmp('2.1.100', '2.1.99') > 0);
  console.log('self-test ok');
}

async function fetchText(page) {
  const r = await fetch(DOCS + page);
  if (!r.ok) throw new Error(`${DOCS}${page} -> HTTP ${r.status}`);
  return r.text();
}

async function main() {
  const args = process.argv.slice(2);
  if (args.includes('--self-test')) return selfTest();
  const write = args.includes('--write');
  const file = args.find((a) => !a.startsWith('--')) ?? join(import.meta.dirname, '..', 'src-tauri', 'defaults', 'settings', 'lazygophers.json');

  let settings, env;
  try {
    [settings, env] = await Promise.all([fetchText('settings-reference.md'), fetchText('env-vars.md')]).then(([a, b]) => [parseSettings(a), parseEnv(b)]);
  } catch (e) {
    console.error(`抓取失败: ${e.message}${e.cause ? ` (${e.cause.code ?? e.cause})` : ''}`);
    process.exit(2);
  }

  const text = readFileSync(file, 'utf8');
  const preset = JSON.parse(text);
  const rows = [];
  const miss = [];
  for (const p of leafPaths(preset)) {
    if (p === 'minimumVersion') continue;
    // _aidog_claude_json 段的键是 ~/.claude.json 的 Global config 键，同样在 settings-reference 小节里查版本
    const hit = lookup(p.replace(/^_aidog_claude_json\./, ''), settings);
    if (hit?.ver) rows.push([p, hit.ver, 'settings-reference#' + hit.key]);
    else miss.push(p + (hit ? ' (小节无版本)' : ''));
  }
  for (const k of Object.keys(preset.env ?? {})) {
    const ver = env.get(k);
    if (ver) rows.push([`env.${k}`, ver, 'env-vars']);
    else miss.push(`env.${k}` + (env.has(k) ? ' (行无版本)' : ''));
  }
  rows.sort((a, b) => cmp(b[1], a[1]));
  for (const r of rows) console.log(r.join(' | '));
  console.log(`\n查到版本 ${rows.length} 个键；无版本/未收录 ${miss.length} 个: ${miss.join(', ')}`);
  const max = rows[0]?.[1];
  if (!max) {
    console.error('没有任何键命中版本，疑似文档格式变化');
    process.exit(2);
  }
  console.log(`计算值 minimumVersion = ${max}`);

  const cur = preset.minimumVersion;
  if (write) {
    if (cur === max) return console.log('已是最新，未改文件');
    const line = /^(\s*)"minimumVersion":.*$/m;
    const anchor = /^(\s*)"autoUpdatesChannel":.*$/m;
    let next;
    if (line.test(text)) next = text.replace(line, `$1"minimumVersion": "${max}",`);
    else if (anchor.test(text)) next = text.replace(anchor, (m, ind) => `${m}\n${ind}"minimumVersion": "${max}",`);
    else throw new Error('找不到 autoUpdatesChannel 锚点');
    JSON.parse(next);
    writeFileSync(file, next);
    console.log(`已写入 ${file}: ${cur ?? '(无)'} -> ${max}`);
    return;
  }
  if (!cur || cmp(cur, max) < 0) {
    console.error(`文件 minimumVersion=${cur ?? '(无)'} < 计算值 ${max}，跑 yarn sync:preset-min-version`);
    process.exit(1);
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(2);
});

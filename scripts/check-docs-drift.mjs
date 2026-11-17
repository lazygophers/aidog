#!/usr/bin/env node
// 文档漂移检测：文档可核对声明 vs 代码真值。
// 对比面见 docs-refresh 工作流「触发」节表格（命令名 / 数字 / 协议 / 路径链接 / 截图时序）。
// 退出码：0 = 无漂移，1 = 有漂移，2 = 环境错误（真值源缺失 / 损坏，不可与「有漂移」混判）。
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const drifts = [];
const fatalErrors = [];

function envError(file, reason) {
  fatalErrors.push(`ERROR: ${file} 读取失败: ${reason}`);
}

function drift(category, loc, claim, reality) {
  drifts.push({ category, loc, claim, reality });
}

function readFile(p) {
  return fs.readFileSync(p, 'utf8');
}

function git(args, cwd = ROOT) {
  return execFileSync('git', args, { cwd, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 }).trim();
}

// ---------- 收集文档文件 ----------
function walkMd(dir, out = []) {
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return out;
  }
  for (const e of entries) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) walkMd(p, out);
    else if (e.name.endsWith('.md') && e.name !== 'README.md') out.push(p);
  }
  return out;
}

const docFiles = [
  ...fs.readdirSync(ROOT).filter((f) => /^README.*\.md$/.test(f)).map((f) => path.join(ROOT, f)),
  path.join(ROOT, 'GLOSSARY.md'),
  ...walkMd(path.join(ROOT, 'docs', 'agents')),
  ...walkMd(path.join(ROOT, 'docs', 'docs')),
];

// 去除 fenced code block（避免把代码示例当文档正文），保留行号。
function stripFences(content) {
  const lines = content.split('\n');
  const out = [];
  let inFence = false;
  for (const line of lines) {
    if (/^\s*```/.test(line)) {
      inFence = !inFence;
      out.push('');
      continue;
      }
    out.push(inFence ? '' : line);
  }
  return out;
}

// ---------- 真值源 ----------
function commandSet() {
  let src;
  try {
    src = readFile(path.join(ROOT, 'src-tauri', 'src', 'startup.rs'));
  } catch (e) {
    envError('src-tauri/src/startup.rs', e.message);
    return new Set();
  }
  const m = src.match(/tauri::generate_handler!\[([\s\S]*?)\]/);
  if (!m) return new Set();
  const names = new Set();
  for (const entry of m[1].split(/[\n,]/)) {
    const id = entry.trim().split('::').pop();
    if (id && /^[a-z_][a-z0-9_]*$/.test(id)) names.add(id);
  }
  return names;
}

function protocolWords() {
  let src;
  try {
    src = readFile(path.join(ROOT, 'src-tauri', 'crates', 'aidog_db', 'src', 'models', 'protocol.rs'));
  } catch (e) {
    envError('src-tauri/crates/aidog_db/src/models/protocol.rs', e.message);
    return new Set();
  }
  const words = new Set();
  for (const m of src.matchAll(/serde\(rename\s*=\s*"([^"]+)"\)/g)) words.add(m[1]);
  return words;
}

function registryCounts() {
  let idx;
  try {
    idx = JSON.parse(readFile(path.join(ROOT, 'src-tauri', 'defaults', 'registry', 'index.json')));
  } catch (e) {
    envError('src-tauri/defaults/registry/index.json', e.message);
    return { platforms: 0, models: 0 };
  }
  const platforms = Array.isArray(idx.platforms) ? idx.platforms.length : 0;
  let models = 0;
  const base = path.join(ROOT, 'src-tauri', 'defaults', 'registry', 'platforms');
  try {
    for (const plat of fs.readdirSync(base)) {
      const mdir = path.join(base, plat, 'models');
      try {
        models += fs.readdirSync(mdir).filter((f) => f.endsWith('.json')).length;
        }
      catch { /* 平台无 models 目录 */ }
    }
  } catch { /* registry 目录缺位 */ }
  return { platforms, models };
}

const commands = commandSet();
const protocols = protocolWords();
const { platforms: ACTUAL_PLATFORMS, models: ACTUAL_MODELS } = registryCounts();

// 代码标识符全量（用于区分「已删除命令」与「文档提到的其他代码符号」）
function codeIdentifiers() {
  const out = new Set();
  const targets = ['src', 'src-tauri/crates', 'src-tauri/src', 'scripts'];
  for (const t of targets) {
    let listing;
    try {
      listing = git(['ls-files', t]);
    } catch {
      continue;
    }
    for (const f of listing.split('\n')) {
      if (!/\.(rs|ts|tsx|js|mjs|jsx)$/.test(f)) continue;
      let content;
      try {
        content = readFile(path.join(ROOT, f));
      } catch {
        continue;
      }
      for (const m of content.matchAll(/[A-Za-z_][A-Za-z0-9_]*/g)) out.add(m[0]);
    }
  }
  return out;
}
const codeIds = codeIdentifiers();
const pkgJson = readFile(path.join(ROOT, 'package.json'));

// 仓库目录名（避免把目录名当陈旧命令，如 docs/doc_build）
function repoDirNames() {
  const names = new Set();
  function walkDir(d, depth) {
    if (depth > 4 || /node_modules|target|\.git|\.tmp|\.worktrees|dist/.test(d)) return;
    let entries;
    try {
      entries = fs.readdirSync(d, { withFileTypes: true });
    } catch {
      return;
    }
    for (const e of entries) {
      if (!e.isDirectory()) continue;
      names.add(e.name);
      walkDir(path.join(d, e.name), depth + 1);
    }
  }
  walkDir(ROOT, 0);
  return names;
}
const dirNames = repoDirNames();

// md 文件所在文档站的根（Rspress：docs/docs；README：仓库根）
function siteRoots(file) {
  const rel = path.relative(ROOT, file);
  if (rel.startsWith('docs' + path.sep)) {
    return [
      path.join(ROOT, 'docs', 'docs'),
      path.join(ROOT, 'docs', 'docs', 'en'),
      path.join(ROOT, 'docs', 'docs', 'zh'),
    ];
  }
  return [ROOT];
}

// ---------- 逐文件扫描 ----------
for (const file of docFiles) {
  const rel = path.relative(ROOT, file);
  let content;
  try {
    content = readFile(file);
  } catch (e) {
    envError(rel, e.message);
    continue;
  }
  const stripped = stripFences(content);
  const lines = stripped; // 与 content 同行号对齐
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (!line) continue;
    const lineNo = i + 1;

    // 类别 1：反引号 snake_case 词 → 命令集合差
    for (const m of line.matchAll(/`([A-Za-z0-9_]+)`/g)) {
      const tok = m[1];
      if (!/^[a-z][a-z0-9]*(_[a-z0-9]+)+$/.test(tok)) continue;
      if (commands.has(tok)) continue;
      if (codeIds.has(tok)) continue; // 是活跃代码符号，不是陈旧命令名
      if (dirNames.has(tok)) continue; // 是仓库目录名
      if (pkgJson.includes(tok)) continue; // 是 package.json scripts / 命令参数（如 cargo test 过滤器 export_bindings）
      drift('命令名', `${rel}:${lineNo}`, `\`${tok}\``, `不在 startup.rs 的 tauri::generate_handler![...] 集合（${commands.size} 个命令），且代码中无该标识符`);
    }

    // 类别 2：数字声明（平台 / 模型数）
    for (const m of line.matchAll(/(\d+)\s*(\+)?\s*(个)?\s*(平台|模型|platforms?|models?)/g)) {
      const n = Number(m[1]);
      const plus = !!m[2];
      const noun = m[4].startsWith('平台') || m[4].startsWith('platform') ? 'platform' : 'model';
      const actual = noun === 'platform' ? ACTUAL_PLATFORMS : ACTUAL_MODELS;
      const bad = plus ? n > actual : n !== actual;
      if (bad) {
        drift('数字', `${rel}:${lineNo}`, m[0].trim(), `registry index.json 现值：${ACTUAL_PLATFORMS} 个平台 / ${ACTUAL_MODELS} 个模型条目`);
      }
    }

    // 类别 3：协议词（snake_case 词出现在含「协议/protocol」的行）
    if (/协议|protocol/i.test(line)) {
      for (const m of line.matchAll(/`([A-Za-z0-9_]+)`/g)) {
        const tok = m[1];
        if (!/^[a-z][a-z0-9]*(_[a-z0-9]+)+$/.test(tok)) continue;
        if (protocols.has(tok)) continue;
        if (codeIds.has(tok)) continue;
        drift('协议', `${rel}:${lineNo}`, `\`${tok}\``, `不在 protocol.rs Protocol 枚举 serde rename 词表（${protocols.size} 个）`);
      }
    }

    // 类别 4：反引号路径存在性（含 / 的 token，截到仓库内）
    const urls = [...line.matchAll(/https?:\/\/[^\s`)]+/g)].map((u) => u[0]);
    for (const m of line.matchAll(/`([^`]+)`/g)) {
      const tok = m[1];
      if (!tok.includes('/')) continue;
      if (/^(https?:|mailto:|data:|~)/.test(tok) || tok.includes('://')) continue;
      if (tok.startsWith('/')) continue; // URL / API 路径片段，非仓库路径
      if (urls.some((u) => u.includes(tok))) continue; // 是外链 URL 的一部分
      const bare = tok.split('#')[0].trim();
      if (!bare || /\s/.test(bare) || /[<>*]/.test(bare)) continue;
      // ADR 引用惯例：`docs/adr/NNNN` 按编号前缀指向 docs/adr/NNNN-*.md，不要求完整文件名
      const adrPrefix = bare.match(/^docs\/adr\/(\d+)$/);
      if (adrPrefix && fs.existsSync(path.join(ROOT, 'docs', 'adr')) && fs.readdirSync(path.join(ROOT, 'docs', 'adr')).some((f) => f.startsWith(`${adrPrefix[1]}-`))) continue;
      // 首段必须落在某个候选根内（否则按外站 URL 尾段跳过）
      const first = bare.split('/')[0];
      const firstCands = [path.resolve(ROOT, first), path.resolve(path.dirname(file), first), ...siteRoots(file).map((r) => path.resolve(r, first))];
      if (!firstCands.some((c) => fs.existsSync(c))) continue;
      const cands = [
        path.resolve(ROOT, bare),
        path.resolve(path.dirname(file), bare),
        ...siteRoots(file).map((r) => path.resolve(r, bare)),
      ];
      if (!cands.some((c) => fs.existsSync(c))) {
        drift('路径', `${rel}:${lineNo}`, `\`${tok}\``, `仓库内不存在（试过 repo 根 / 文档目录 / 文档站根相对）`);
      }
   }
  }

  // 类别 4b：Markdown 相对链接
  const raw = content;
  let inFence = false;
  for (const [idx, line] of raw.split('\n').entries()) {
    if (/^\s*```/.test(line)) inFence = !inFence;
    if (inFence) continue;
    for (const m of line.matchAll(/\[[^\]]*\]\(([^)]+)\)/g)) {
      const target = m[1].trim();
      if (/^(https?:|mailto:|#|\/\/|[a-z]+:)/i.test(target)) continue;
      const bare = target.split('#')[0].trim();
      if (!bare) continue;
      const bases = [path.dirname(file), ROOT, ...siteRoots(file)];
      const cands = [];
      for (const b of bases) {
        const relBare = bare.replace(/^\/+/, ''); // 站根绝对链接按文档站根解析
        cands.push(path.resolve(b, relBare), path.resolve(b, relBare) + '.md', path.resolve(b, relBare, 'index.md'), path.resolve(b, relBare) + '.mdx', path.resolve(b, relBare, 'index.mdx'));
      }
      if (!cands.some((c) => fs.existsSync(c))) {
        drift('链接', `${rel}:${idx + 1}`, `[...](${target})`, `目标不存在（试过相对目录 / repo 根 / 文档站根，含 .md/.mdx 与 index 变体）`);
      }
    }
  }
}

// ---------- 类别 5：截图 vs src/ 最近提交 ----------
function lastCommitTs(argPath) {
  try {
    const out = git(['log', '-1', '--format=%ct', '--', argPath]);
    return out ? Number(out) : null;
  } catch {
    return null;
  }
}
const srcTs = lastCommitTs('src');
const imgDirs = ['assets', 'screenshots', path.join('docs', 'docs', 'public')];
const images = [];
for (const d of imgDirs) {
  const abs = path.join(ROOT, d);
  if (!fs.existsSync(abs)) continue;
  for (const e of fs.readdirSync(abs, { withFileTypes: true })) {
    if (e.isFile() && /\.(png|jpe?g|webp|gif)$/i.test(e.name)) images.push(path.join(d, e.name));
  }
}
// logo / 图标类图片不是产品截图，跳过
const productShots = images.filter((i) => !/logo|icon|favicon/i.test(path.basename(i)));
for (const img of productShots) {
  let imgTs = lastCommitTs(img);
  if (imgTs === null) imgTs = Math.floor(fs.statSync(path.join(ROOT, img)).mtimeMs / 1000);
  // shortcut: fresh clone 后 mtime=checkout 时刻恒晚于 src 提交时间，陈旧截图漏报; 升级为按 git log 取截图提交时间 when 需要在 fresh clone 上可靠
  if (srcTs !== null && imgTs < srcTs) {
    drift('截图', img, `截图（${new Date(imgTs * 1000).toISOString()}）`, `src/ 最近 UI 提交 ${new Date(srcTs * 1000).toISOString()} 之后有 UI 变更 → 截图可能过时`);
  }
}

// ---------- 输出 ----------
for (const e of fatalErrors) console.error(e);
if (drifts.length === 0 && fatalErrors.length === 0) {
  console.log('无漂移（EXIT=0）');
} else {
  const byCat = {};
  for (const d of drifts) byCat[d.category] = (byCat[d.category] || 0) + 1;
  console.log(`漂移共 ${drifts.length} 条：` + Object.entries(byCat).map(([k, v]) => `${k} ${v}`).join(' / '));
  for (const d of drifts) {
    console.log(`[${d.category}] ${d.loc} → ${d.claim} → ${d.reality}`);
  }
}
process.exitCode = fatalErrors.length ? 2 : drifts.length ? 1 : 0;

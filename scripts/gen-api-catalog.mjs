#!/usr/bin/env node
/** 「API」分页接口清单：从 rpc_routes! 命令表 + Rust 签名生成 src/generated/apiCatalog.json。
 *  yarn gen:api-catalog 生成；yarn check:api-catalog 比对（改命令不重新生成即红）。 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parseRpcTable } from "./lib/rpc-table.mjs";

const root = path.resolve(import.meta.dirname, "..");
const OUT = path.join(root, "src/generated/apiCatalog.json");
const camel = (s) => s.replace(/_([a-z0-9])/g, (_, c) => c.toUpperCase());
const IPC_ONLY = /AppHandle|Window|Webview|State</; // 桌面注入参数，不是调用方传的

/** 从 `(` 起配平括号，返回 [括号内文本, 结束下标]。 */
function balanced(src, open) {
  let d = 0;
  for (let i = open; i < src.length; i++) {
    if (src[i] === "(") d++;
    else if (src[i] === ")" && --d === 0) return [src.slice(open + 1, i), i];
  }
  throw new Error("括号未配平");
}

/** 顶层逗号切分（忽略 <> () [] 内）。 */
function splitTop(s) {
  const out = [];
  let d = 0, cur = "";
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if ("<([".includes(c)) d++;
    else if (")]".includes(c) || (c === ">" && s[i - 1] !== "-")) d--;
    if (c === "," && d === 0) { out.push(cur); cur = ""; } else cur += c;
  }
  if (cur.trim()) out.push(cur);
  return out.map((x) => x.trim()).filter(Boolean);
}

export function parseSignature(src, name) {
  const m = src.match(new RegExp(`\\bfn\\s+${name}\\s*\\(`));
  if (!m) return null;
  const open = m.index + m[0].length - 1;
  const [inside, end] = balanced(src, open);
  const params = splitTop(inside.replace(/\/\/[^\n]*/g, ""))
    .map((p) => { const i = p.indexOf(":"); return { name: p.slice(0, i).trim().replace(/^mut\s+/, ""), type: p.slice(i + 1).trim().replace(/\s+/g, " ") }; })
    .filter((p) => !IPC_ONLY.test(p.type))
    .map((p) => ({ name: camel(p.name), type: p.type }));
  const rest = src.slice(end + 1, src.indexOf("{", end));
  const rm = rest.match(/->\s*([\s\S]+?)\s*$/);
  let returns = rm ? rm[1].replace(/\s+/g, " ") : "()";
  const res = returns.match(/^Result<([\s\S]+),\s*[^,]+>$/);
  if (res) returns = res[1].trim();
  return { params, returns };
}

export function categoryOf(rpcPath) {
  const segs = rpcPath.split("::").slice(0, -1);
  const first = segs[0] === "aidog_core" ? segs[1] : segs[0];
  return first.replace(/^aidog_/, "").replace(/_cmd$/, "");
}

/** 纯函数：rpcSrc = aidog_rpc/src/lib.rs；rustSrcs = 全部 .rs 源文本；startupSrc = startup.rs。 */
export function buildCatalog(rpcSrc, rustSrcs, startupSrc) {
  const { entries } = parseRpcTable(rpcSrc);
  const sigOf = (name) => {
    for (const s of rustSrcs) { const r = parseSignature(s, name); if (r) return r; }
    throw new Error(`找不到命令 ${name} 的 Rust 签名`);
  };
  const rows = entries.map((e) => ({
    name: e.name, category: categoryOf(e.path), kind: e.kind, exportsSecret: e.exportsSecret, desktopOnly: false, ...sigOf(e.name),
  }));
  const h = startupSrc.match(/generate_handler!\s*\[([\s\S]*?)\]\s*\)/);
  const registered = [...h[1].matchAll(/\b(aidog_[a-z0-9_]+(?:::[A-Za-z0-9_]+)+)\s*,?/g)].map((m) => m[1]);
  const known = new Set(rows.map((r) => r.name));
  for (const p of registered) {
    const name = p.split("::").pop();
    if (known.has(name)) continue;
    rows.push({ name, category: categoryOf(p), kind: null, exportsSecret: false, desktopOnly: true, ...sigOf(name) });
  }
  rows.sort((a, b) => a.name.localeCompare(b.name));
  return { commands: rows };
}

function rustFiles(dir) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const f = path.join(dir, e.name);
    if (e.isDirectory()) return e.name === "target" || e.name === "node_modules" ? [] : rustFiles(f);
    return f.endsWith(".rs") ? [f] : [];
  });
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const rd = (f) => fs.readFileSync(path.join(root, f), "utf8");
  const gen = JSON.stringify(
    buildCatalog(rd("src-tauri/crates/aidog_rpc/src/lib.rs"), rustFiles(path.join(root, "src-tauri")).map((f) => fs.readFileSync(f, "utf8")), rd("src-tauri/src/startup.rs")),
    null, 2,
  ) + "\n";
  if (process.argv.includes("--check")) {
    if (!fs.existsSync(OUT) || fs.readFileSync(OUT, "utf8") !== gen) {
      console.error("src/generated/apiCatalog.json 与代码不一致，请运行 yarn gen:api-catalog");
      process.exit(1);
    }
    console.log("api catalog 检查通过");
  } else {
    fs.mkdirSync(path.dirname(OUT), { recursive: true });
    fs.writeFileSync(OUT, gen);
    console.log(`已生成 ${path.relative(root, OUT)}`);
  }
}

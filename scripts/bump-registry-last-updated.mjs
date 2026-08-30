#!/usr/bin/env node
// registry last_updated 盖戳器：对本次 git 变更涉及的 registry json 文件把顶层 last_updated
// 盖成当前 Unix 秒，并同步推高 index.json 的全局 last_updated（整轮早退判据）。
//
// 纯文本插入/替换，不 JSON round-trip（保科学计数法等字面写法——registry 手维护，禁机器改写内容）；
// 跳过 schema/（JSON Schema 文档非数据文件）。
//
// 用法：
//   node scripts/bump-registry-last-updated.mjs          # 只盖 git 变更（含 untracked）的文件
//   node scripts/bump-registry-last-updated.mjs --all    # 全量盖（初始化 / 一次性补齐用）
//
// 提交前跑：改了 platform.json / models/*.json 而时间戳没变新 → 同步永远跳过该文件。
import { execSync } from "node:child_process";
import { readFileSync, writeFileSync, statSync, readdirSync } from "node:fs";
import { join, relative } from "node:path";

const ROOT = join(import.meta.dirname, "..");
const REG = join(ROOT, "src-tauri/defaults/registry");
const ALL = process.argv.includes("--all");

const listRegistryFiles = () => {
  const out = [];
  const walk = (dir) => {
    for (const name of readdirSync(dir)) {
      const p = join(dir, name);
      if (statSync(p).isDirectory()) walk(p);
      else if (name.endsWith(".json")) out.push(p);
    }
  };
  walk(REG);
  // schema/ 是 JSON Schema 文档不是数据，不盖戳
  return out.filter((p) => !relative(REG, p).startsWith("schema/"));
};

const changed = new Set(
  execSync("git status --porcelain -- src-tauri/defaults/registry", { cwd: ROOT })
    .toString()
    .split("\n")
    .filter(Boolean)
    .map((l) => join(ROOT, l.slice(3).trim()))
    // schema/ 是 JSON Schema 文档不是数据，不盖戳（同一过滤与 listRegistryFiles 对齐）
    .filter((p) => !relative(REG, p).startsWith("schema/")),
);

/** 纯文本盖戳：顶层已有 last_updated → 整行替换数值；没有 → 紧跟开括号后插入首个键。
 *  前置假设（check-registry 保）：文件 pretty-print 2 空格缩进、`{` 独占首行。
 *  改完重新 JSON.parse 校验：结果与原文档 deep-equal 且仅多/换 last_updated，失败即放弃。 */
const stamp = (p) => {
  const raw = readFileSync(p, "utf8");
  const before = JSON.parse(raw);
  const now = Math.floor(Date.now() / 1000);
  if (before.last_updated === now) return false;

  let next;
  if (Object.hasOwn(before, "last_updated")) {
    next = raw.replace(/^(\s*)"last_updated": -?\d+(,?)$/m, `$1"last_updated": ${now}$2`);
  } else {
    if (!raw.startsWith("{\n")) throw new Error("非预期格式：首行不是单独的 '{'");
    next = raw.replace(/^\{\n/, `{\n  "last_updated": ${now},\n`);
  }

  const after = JSON.parse(next);
  const expected = { ...before, last_updated: now };
  // 键序无关的规范化比较（数组保序）：文本未被改写，数值/结构必须与预期完全一致
  const canon = (v) =>
    v === null || typeof v !== "object"
      ? JSON.stringify(v)
      : Array.isArray(v)
        ? "[" + v.map(canon).join(",") + "]"
        : "{" + Object.keys(v).sort().map((k) => JSON.stringify(k) + ":" + canon(v[k])).join(",") + "}";
  if (canon(after) !== canon(expected)) {
    throw new Error("盖戳后文档与预期不一致，放弃（可能非 2 空格缩进或含重复键）");
  }
  writeFileSync(p, next);
  return true;
};

const targets = ALL ? listRegistryFiles() : [...changed];
let stamped = 0;
const errs = [];
for (const p of targets) {
  const rel = relative(ROOT, p);
  if (rel.endsWith("index.json")) continue; // 最后统一盖
  try {
    if (stamp(p)) stamped++;
  } catch (e) {
    errs.push(`${rel}: ${e.message}`);
  }
}

// 任一 registry 文件被盖 → index.json 全局 last_updated 同步推高（整轮早退判据，
// 不推高则远程比对本地 registry_last_updated 后整轮跳过，新内容永远进不来）。
if (stamped > 0 || ALL) stamp(join(REG, "index.json"));

console.log(`bump-registry-last-updated: ${stamped} 个文件已盖戳${ALL ? "（--all 全量）" : ""}`);
for (const e of errs) console.error(`skip ${e}`);
if (errs.length) process.exitCode = 1;

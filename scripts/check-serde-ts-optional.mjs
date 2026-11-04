// ts-rs 导出的 TS 类型里，skip_serializing_if 的字段必须是可选（`?:`），
// 否则序列化时缺省的键在前端读到 undefined 却被类型声明为必有（8db8c455e 日志详情崩溃）。
// 规则：derive(TS) 的结构体里，带 skip_serializing_if 的字段须带 #[ts(optional…)] 或 #[ts(skip)]，
// 或结构体带 #[ts(optional_fields)]。shortcut: 行级扫描，不解析 cfg_attr / 枚举变体字段。
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export function checkSource(src, file) {
  const lines = src.split("\n");
  const errors = [];
  for (let i = 0; i < lines.length; i++) {
    if (!/^\s*#\[derive\(.*\bTS\b/.test(lines[i])) continue;
    let j = i;
    let structAttrs = "";
    while (j < lines.length && !/^\s*(pub(\([^)]*\))?\s+)?struct\s/.test(lines[j])) {
      structAttrs += lines[j++] + "\n";
      if (/^\s*(pub(\([^)]*\))?\s+)?(enum|fn|impl)\s/.test(lines[j] ?? "")) { j = -1; break; }
    }
    if (j < 0 || j >= lines.length || !lines[j].includes("{")) continue; // 枚举 / 元组结构体跳过
    const structOptional = /ts\([^)]*optional_fields/.test(structAttrs);
    let attrs = "";
    for (let k = j + 1; k < lines.length && !/^\}/.test(lines[k]); k++) {
      const l = lines[k];
      if (/^\s*(#\[|\/\/)/.test(l) || /^\s*$/.test(l)) { attrs += l + "\n"; continue; }
      const m = l.match(/^\s*(?:pub(?:\([^)]*\))?\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*:/);
      if (m && attrs.includes("skip_serializing_if") && !structOptional &&
          !/ts\([^)]*\b(optional|skip)\b/.test(attrs)) {
        errors.push(`${file}:${k + 1}: 字段 ${m[1]} 带 skip_serializing_if 但缺 #[ts(optional)]`);
      }
      attrs = "";
    }
  }
  return errors;
}

function walk(dir, out) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) { if (e.name !== "target") walk(p, out); }
    else if (e.name.endsWith(".rs")) out.push(p);
  }
  return out;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../src-tauri");
  const errors = walk(root, []).flatMap((f) => checkSource(fs.readFileSync(f, "utf8"), path.relative(root, f)));
  if (errors.length) { console.error(errors.join("\n")); process.exit(1); }
  console.log("check-serde-ts-optional: ok");
}

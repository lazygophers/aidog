// 票 06 零差集证明（一）：从 `src-tauri/src/startup.rs` 的 `tauri::generate_handler![...]`
// 抽出注册的命令名集合，排序后逐行输出。
//
// 用法：node scripts/t06-handler-names.mjs [startup.rs 路径]
// 默认读工作区当前文件；改造前先 `git stash` / `git show <ref>:...` 出旧版本再跑一次比对。
import { readFileSync } from "fs";

const path = process.argv[2] ?? "src-tauri/src/startup.rs";
const src = readFileSync(path, "utf8");

const marker = "generate_handler![";
const start = src.indexOf(marker);
if (start < 0) throw new Error(`generate_handler! not found in ${path}`);

// 从 `[` 起做方括号配平，拿到完整宏参数体（内含注释与多层路径）。
let i = start + marker.length - 1;
let depth = 0;
let end = -1;
for (let j = i; j < src.length; j++) {
  if (src[j] === "[") depth++;
  else if (src[j] === "]") {
    depth--;
    if (depth === 0) {
      end = j;
      break;
    }
  }
}
if (end < 0) throw new Error("unbalanced generate_handler! brackets");

const body = src
  .slice(i + 1, end)
  // 去行注释与块注释，防止注释里的示例名污染集合
  .replace(/\/\*[\s\S]*?\*\//g, "")
  .replace(/\/\/[^\n]*/g, "");

const names = body
  .split(",")
  .map((s) => s.trim())
  .filter(Boolean)
  // invoke 名 = `#[tauri::command]` 函数名，与模块路径无关 → 取路径最后一段
  .map((s) => s.split("::").pop());

const uniq = [...new Set(names)].sort();
if (uniq.length !== names.length) {
  console.error(`WARN: duplicate entries in generate_handler! (${names.length} -> ${uniq.length})`);
}
console.error(`# ${path}: ${uniq.length} commands`);
console.log(uniq.join("\n"));

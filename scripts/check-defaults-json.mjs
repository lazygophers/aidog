// 语法门禁：src-tauri/defaults/ 下所有 JSON 必须 JSON.parse 通过且无重复键。
// registry/ 另有 check-registry（schema + 字段），此处只兜语法——非法 JSON 编译期
// include_str! 不报错，启动灌种才炸（2026-11-02 agent 半成品实锤）。
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { findDuplicateKeys } from './json-dup-keys.mjs';

const root = join(import.meta.dirname, '..', 'src-tauri', 'defaults');
const files = [];

function walk(dir) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) walk(p);
    else if (name.endsWith('.json')) files.push(p);
  }
}
walk(root);

let bad = 0;
for (const p of files) {
  const text = readFileSync(p, 'utf8');
  try {
    JSON.parse(text);
  } catch (e) {
    console.error(`${p}: 语法错误: ${e.message}`);
    bad++;
    continue;
  }
  for (const d of findDuplicateKeys(text)) {
    console.error(`${p}: 重复键 "${d.key}" 第 ${d.firstLine} 与 ${d.line} 行`);
    bad++;
  }
}

console.log(`check-defaults-json: ${files.length} 个文件, ${bad} 个错误`);
process.exit(bad ? 1 : 0);

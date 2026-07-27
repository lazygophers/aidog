#!/usr/bin/env node
// 契约止血闸门：Rust #[derive(Serialize/Deserialize)] struct 字段集 ↔ TS interface 字段集比对。
// 策略：按结构体/接口同名匹配（gateway/models/*.rs 的 struct name === types/*.ts 的 interface name），
// 正则轻量解析字段名做双向差集。非自动推断跨语言对应关系——只信同名，改名的结构体互相看不见彼此，
// 需要跨名映射时在 MANUAL_ALIASES 里补一条。
// ponytail: 正则非真解析器，遇嵌套 struct/枚举字段类型里的花括号会误判——目前 models/*.rs 里没有这种字段，先够用。

import { readFileSync, readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, "..");
const RUST_DIR = join(ROOT, "src-tauri/crates/aidog_core/src/gateway/models");
const TS_DIR = join(ROOT, "src/services/api/types");

// Rust struct 名 → TS interface 名，仅在两侧命名不同时才需要填。
const MANUAL_ALIASES = {};

// 已知合理的单侧字段（有明确理由不该在比对中报错的），格式 "StructName.field_name"。
// 每条必须有注释说明理由——这是止血闸门，不是用来消音真漂移的。
const IGNORE = new Set([
  // balance_level: Platform 上 #[serde(skip_deserializing)]，只出不进；TS 已标 `balance_level?` 覆盖序列化侧,故不视为缺失。
]);

function extractRustStructs(dir) {
  const structs = {};
  for (const f of readdirSync(dir)) {
    if (!f.endsWith(".rs") || f.startsWith("test_")) continue;
    const src = readFileSync(join(dir, f), "utf8");
    // 找每个 #[derive(...)] 含 Serialize 或 Deserialize，后面跟着的 pub struct Name { ... }
    const re = /#\[derive\(([^)]*)\)\][^\n]*\n(?:[^\n]*\n)*?\s*pub struct (\w+)\s*\{([\s\S]*?)\n\}/g;
    let m;
    while ((m = re.exec(src))) {
      const [, derive, name, body] = m;
      if (!/Serialize|Deserialize/.test(derive)) continue;
      const fields = [];
      for (const line of body.split("\n")) {
        const fm = line.match(/^\s*pub\s+(\w+)\s*:/);
        if (!fm) continue;
        fields.push(fm[1]);
      }
      structs[name] = { fields, file: f };
    }
  }
  return structs;
}

function extractTsInterfaces(dir) {
  const interfaces = {};
  for (const f of readdirSync(dir)) {
    if (!f.endsWith(".ts")) continue;
    const src = readFileSync(join(dir, f), "utf8");
    const re = /export interface (\w+)\s*\{([\s\S]*?)\n\}/g;
    let m;
    while ((m = re.exec(src))) {
      const [, name, body] = m;
      const fields = [];
      for (const line of body.split("\n")) {
        const fm = line.match(/^\s*(?:\/\*\*.*\*\/\s*)?(\w+)\??\s*:/);
        if (!fm) continue;
        fields.push(fm[1]);
      }
      interfaces[name] = { fields, file: f };
    }
  }
  return interfaces;
}

const rustStructs = extractRustStructs(RUST_DIR);
const tsInterfaces = extractTsInterfaces(TS_DIR);

let mismatches = 0;
for (const [structName, { fields: rustFields, file: rustFile }] of Object.entries(rustStructs)) {
  const tsName = MANUAL_ALIASES[structName] ?? structName;
  const tsEntry = tsInterfaces[tsName];
  if (!tsEntry) continue; // 无同名 TS interface：该 struct 不跨 IPC 边界暴露给前端，跳过。
  const { fields: tsFields, file: tsFile } = tsEntry;
  const rustSet = new Set(rustFields);
  const tsSet = new Set(tsFields);

  const missingInTs = rustFields.filter((f) => !tsSet.has(f) && !IGNORE.has(`${structName}.${f}`));
  const missingInRust = tsFields.filter((f) => !rustSet.has(f) && !IGNORE.has(`${structName}.${f}`));

  if (missingInTs.length || missingInRust.length) {
    mismatches++;
    console.log(`\n✗ ${structName}  (${rustFile} ↔ ${tsFile})`);
    if (missingInTs.length) console.log(`  Rust 有, TS 缺: ${missingInTs.join(", ")}`);
    if (missingInRust.length) console.log(`  TS 有, Rust 缺: ${missingInRust.join(", ")}`);
  }
}

const pairCount = Object.keys(rustStructs).filter((n) => tsInterfaces[MANUAL_ALIASES[n] ?? n]).length;
console.log(`\n比对 struct/interface 对数: ${pairCount}, 不一致: ${mismatches}`);

process.exit(mismatches > 0 ? 1 : 0);

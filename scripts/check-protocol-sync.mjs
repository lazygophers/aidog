#!/usr/bin/env node
// Protocol Rust↔TS 跨层词表同步门禁（对照 scripts/check-registry.mjs 风格：失败一次列全部不匹配项，exit 1）。
// ① protocol.rs Protocol 枚举 `#[serde(rename = "<code>")]` 变体 ↔ manual.ts Protocol 联合类型字面量（双向零差集）。
//    serde rename 一旦发布禁改名（DB 持久值）；漏双写 → Protocol::from_db_str 静默回落 Anthropic，故此检查必须硬错。
// ② protocol.rs endpoints_locked() matches! 变体 ↔ constants.ts ENDPOINTS_LOCKED_PROTOCOLS。
// ③ middleware.rs Target/MatchType/ActionKind 枚举变体（snake_case）↔ MiddlewareRules.tsx
//    TARGETS/MATCH_TYPES/ACTION_KINDS 运行时数组（generated/*.ts 只有类型导出无运行时值，故抽 Rust 源枚举）。
// 用法：node scripts/check-protocol-sync.mjs

import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const root = join(scriptDir, "..");
const read = (p) => readFileSync(join(root, p), "utf8");
const failures = [];

function report(name, rustSide, tsSide, rustOnly, tsOnly) {
  if (!rustOnly.length && !tsOnly.length) {
    console.log(`${name}：一致（Rust ${rustSide} ↔ 前端 ${tsSide}）`);
    return;
  }
  for (const c of rustOnly) failures.push([name, `Rust 多出（前端缺失）：${c}`]);
  for (const c of tsOnly) failures.push([name, `前端多出（Rust 缺失）：${c}`]);
}

// ── ① Protocol 枚举 serde rename ↔ manual.ts 联合类型 ──

const protocolRs = read("src-tauri/crates/aidog_db/src/models/protocol.rs");
const enumBlock = protocolRs.match(/pub enum Protocol \{([\s\S]*?)\n\}/)?.[1];
if (!enumBlock) {
  failures.push(["protocol.rs", "未找到 `pub enum Protocol` 块——文件结构变了？"]);
}
// 变体名 → serde rename 字符串（rename attr 紧邻变体行；doc 注释行跳过）
const variantToCode = new Map();
if (enumBlock) {
  let pending = null;
  for (const line of enumBlock.split("\n")) {
    const attr = line.match(/#\[serde\(rename = "([^"]+)"\)\]/);
    if (attr) {
      pending = attr[1];
      continue;
    }
    const variant = line.match(/^\s+([A-Z][A-Za-z0-9]*)\s*(?:[({=,]|$)/);
    if (variant) {
      if (!pending) {
        failures.push(["protocol.rs", `变体 ${variant[1]} 无 #[serde(rename)]——本脚本假设全枚举显式 rename`]);
      } else {
        variantToCode.set(variant[1], pending);
      }
      pending = null;
    }
  }
}

const manualTs = read("src/services/api/types/manual.ts");
const unionBody = manualTs.match(/export type Protocol =([\s\S]*?);/)?.[1];
if (!unionBody) failures.push(["manual.ts", "未找到 `export type Protocol` 联合类型"]);
const tsLiterals = new Set(unionBody ? [...unionBody.matchAll(/"([^"]+)"/g)].map((m) => m[1]) : []);

const rustCodes = new Set(variantToCode.values());
if (!rustCodes.size || !tsLiterals.size) {
  failures.push(["protocol-sync", `①解析结果为空（Rust ${rustCodes.size} / 前端 ${tsLiterals.size}）——检查文件是否被重构`]);
} else {
  report(
    "① Protocol 枚举（serde rename ↔ manual.ts 联合类型）",
    `${rustCodes.size} 个变体`,
    `${tsLiterals.size} 个字面量`,
    [...rustCodes].filter((c) => !tsLiterals.has(c)).sort(),
    [...tsLiterals].filter((c) => !rustCodes.has(c)).sort(),
  );
}

// ── ② endpoints_locked() matches! 变体 ↔ ENDPOINTS_LOCKED_PROTOCOLS ──

// 有意差异白名单：透传平台无标准 wire endpoint，端点不参与（前端不锁编辑）。
// 依据：docs/adr/0008-passthrough-platform-semantics.md（Rust 侧注释同源）。
const INTENTIONAL_UNLOCKED = new Set(["passthrough"]);

const lockedFn = protocolRs.match(/fn endpoints_locked\(&self\) -> bool \{([\s\S]*?)\n    \}/)?.[1];
const variantSet = new Set(variantToCode.keys());
const lockedCodes = new Set();
if (!lockedFn) {
  failures.push(["protocol.rs", "未找到 `fn endpoints_locked` ——文件结构变了？"]);
} else {
  // 只取 matches!(self, ...) 的变体清单，剥注释后按 | 拆分
  const armList = lockedFn.match(/matches!\(\s*self,([\s\S]*?)\)/)?.[1] ?? "";
  for (const raw of armList.replace(/\/\/[^\n]*/g, "").split("|")) {
    const name = raw.trim();
    if (!name) continue;
    if (!/^[A-Z][A-Za-z0-9]*$/.test(name)) {
      failures.push(["protocol.rs", `endpoints_locked 分支不是变体名：${JSON.stringify(name)}`]);
      continue;
    }
    const code = variantToCode.get(name);
    if (!code) {
      failures.push(["protocol.rs", `endpoints_locked 引用了 Protocol 之外的变体：${name}`]);
      continue;
    }
    lockedCodes.add(code);
  }
}

const constantsTs = read("src/domains/platforms/constants.ts");
const lockedArr = constantsTs.match(
  /export const ENDPOINTS_LOCKED_PROTOCOLS[\s\S]*?new Set<Protocol>\(\[([\s\S]*?)\]\)/,
)?.[1];
if (!lockedArr) failures.push(["constants.ts", "未找到 ENDPOINTS_LOCKED_PROTOCOLS 集合初始化"]);
const tsLocked = new Set(lockedArr ? [...lockedArr.matchAll(/"([^"]+)"/g)].map((m) => m[1]) : []);

if (lockedCodes.size && tsLocked.size) {
  const rustOnly = [...lockedCodes].filter((c) => !tsLocked.has(c) && !INTENTIONAL_UNLOCKED.has(c)).sort();
  const tsOnly = [...tsLocked].filter((c) => !lockedCodes.has(c)).sort();
  const whitelisted = [...lockedCodes].filter((c) => INTENTIONAL_UNLOCKED.has(c)).sort();
  if (!rustOnly.length && !tsOnly.length) {
    console.log(
      `② endpoints_locked：一致（Rust ${lockedCodes.size} ↔ 前端 ${tsLocked.size}${whitelisted.length ? `，白名单豁免 ${whitelisted.join(", ")}（ADR 0008）` : ""}）`,
    );
  } else {
    for (const c of rustOnly)
      failures.push(["endpoints_locked", `Rust 多出（前端缺失）：${c}${INTENTIONAL_UNLOCKED.has(c) ? "" : ""}`]);
    for (const c of tsOnly) failures.push(["endpoints_locked", `前端多出（Rust 缺失）：${c}`]);
    if (whitelisted.length && !rustOnly.length)
      // 白名单项被前端补齐后：把它从 INTENTIONAL_UNLOCKED 删掉，防止白名单静默腐化
      failures.push(["endpoints_locked", `白名单项已被前端收录，请从 INTENTIONAL_UNLOCKED 移除：${whitelisted.join(", ")}`]);
  }
} else if (!lockedFn || !lockedArr) {
  // 已在上面报解析失败
} else {
  failures.push(["endpoints_locked", `②解析结果为空（Rust ${lockedCodes.size} / 前端 ${tsLocked.size}）`]);
}

// ── ③ middleware 词表 ↔ MiddlewareRules.tsx 运行时数组 ──

const middlewareRs = read("src-tauri/crates/aidog_db/src/models/middleware.rs");
const rulesTsx = read("src/components/settings/MiddlewareRules.tsx");
const snake = (name) => name.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase();

for (const [enumName, arrayName] of [
  ["Target", "TARGETS"],
  ["MatchType", "MATCH_TYPES"],
  ["ActionKind", "ACTION_KINDS"],
]) {
  const block = middlewareRs.match(new RegExp(`pub enum ${enumName} \\{([\\s\\S]*?)\\n\\}`))?.[1];
  const arr = rulesTsx.match(new RegExp(`const ${arrayName} = \\[([^\\]]*)\\]`))?.[1];
  if (!block || !arr) {
    failures.push([`${enumName}/${arrayName}`, `解析失败（枚举块 ${!!block} / 数组 ${!!arr}）——检查文件是否被重构`]);
    continue;
  }
  // 枚举均 `#[serde(rename_all = "snake_case")]` 的 unit 变体
  const rustVariants = [
    ...block.matchAll(/^\s+([A-Z][A-Za-z0-9]*)\s*(?:\([^)]*\))?\s*,?\s*$/gm),
  ].map((m) => snake(m[1]));
  const tsVariants = [...arr.matchAll(/"([^"]+)"/g)].map((m) => m[1]);
  if (!rustVariants.length || !tsVariants.length) {
    failures.push([`${enumName}/${arrayName}`, `③解析结果为空（Rust ${rustVariants.length} / 前端 ${tsVariants.length}）`]);
    continue;
  }
  report(
    `③ ${enumName} ↔ ${arrayName}`,
    `${rustVariants.length} 个变体`,
    `${tsVariants.length} 个值`,
    rustVariants.filter((c) => !tsVariants.includes(c)),
    tsVariants.filter((c) => !rustVariants.includes(c)),
  );
}

if (failures.length) {
  console.error(`protocol 跨层同步校验失败：${failures.length} 处`);
  for (const [where, msg] of failures) console.error(`  ${where}: ${msg}`);
  process.exit(1);
}
console.log("protocol 跨层同步校验通过（①枚举双写 ②endpoints_locked ③middleware 词表）");

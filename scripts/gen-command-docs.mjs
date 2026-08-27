#!/usr/bin/env node
/** 从 startup、Rust 实现和 TS wrapper 生成 Tauri command 字典。 */

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = path.resolve(import.meta.dirname, "..");
const startupPath = path.join(root, "src-tauri/src/startup.rs");
const rustRoot = path.join(root, "src-tauri");
const tsRoot = path.join(root, "src/services/api");
const outputPath = path.join(root, "docs/docs/zh/api/commands.generated.mdx");

function filesUnder(dir, suffix) {
  const result = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const file = path.join(dir, entry.name);
    if (entry.isDirectory()) result.push(...filesUnder(file, suffix));
    else if (entry.isFile() && file.endsWith(suffix)) result.push(file);
  }
  return result;
}

function sorted(values) {
  return [...values].sort((a, b) => a.localeCompare(b));
}

function parseStartup(source) {
  const handler = source.match(/generate_handler!\s*\[([\s\S]*?)\]\s*\)/);
  if (!handler) throw new Error("startup.rs 中找不到 generate_handler! 注册表");
  // 注册表条目不止 aidog_core：workspace 拆分后 aidog_backup / aidog_cli_proxy 等 crate
  // 也直接注册 command，写死 `aidog_core::` 会把它们整批漏掉（表现为「TS wrapper 未注册 startup」误报）。
  const entries = [...handler[1].matchAll(/\baidog_[a-z0-9_]+::[A-Za-z0-9_:]+::([A-Za-z0-9_]+)\s*,?/g)];
  if (!entries.length) throw new Error("startup.rs 注册表为空或格式无法解析");
  return new Map(entries.map(([, name]) => [name, "startup.rs"]));
}

function parseRust(files) {
  const commands = new Map();
  for (const file of files) {
    const source = fs.readFileSync(file, "utf8");
    // 宏调用路径两种：crate 内 `crate::tauri_command!`，其他 crate `aidog_core::tauri_command!`。
    for (const match of source.matchAll(/^\s*(?:crate|aidog_core)::tauri_command!\s*\{\s*(?:\/\/[^\n]*\n\s*)*pub\s+(?:async\s+)?fn\s+([A-Za-z0-9_]+)/gm)) {
      commands.set(match[1], path.relative(root, file));
    }
    for (const match of source.matchAll(/^\s*#\[tauri::command(?:\([^\]]*\))?\]\s*(?:#\[[^\n]+\]\s*)*(?:pub\s+)?(?:async\s+)?fn\s+([A-Za-z0-9_]+)/gm)) {
      commands.set(match[1], path.relative(root, file));
    }
  }
  return commands;
}

function parseTs(files) {
  const commands = new Map();
  for (const file of files) {
    const source = fs.readFileSync(file, "utf8");
    for (const match of source.matchAll(/\binvoke(?:\s*<[^;]*?>)?\s*\(\s*["']([^"']+)["']/gs)) {
      commands.set(match[1], path.relative(root, file));
    }
  }
  return commands;
}

function difference(left, right) {
  return sorted([...left].filter((value) => !right.has(value)));
}

function render(startup, rust, ts) {
  const rows = sorted(startup.keys()).map((name) => {
    const internal = !ts.has(name);
    return `| \`${name}\` | ${internal ? "是" : "否"} | \`${rust.get(name)}\` | ${internal ? "无 TS wrapper" : `\`${ts.get(name)}\``} |`;
  });
  return `---
title: Tauri Command 字典
description: 由 startup 注册表、Rust 实现和 TypeScript wrapper 生成的 command 清单。
---

# Tauri Command 字典

此页由 \`scripts/gen-command-docs.mjs\` 生成，请勿手工编辑。

| Command | Internal | Rust 实现 | TypeScript wrapper |
| --- | --- | --- | --- |
${rows.join("\n")}
`;
}

try {
  const startup = parseStartup(fs.readFileSync(startupPath, "utf8"));
  const rust = parseRust(filesUnder(rustRoot, ".rs"));
  const ts = parseTs(filesUnder(tsRoot, ".ts"));
  const startupSet = new Set(startup.keys());
  const rustSet = new Set(rust.keys());
  const tsSet = new Set(ts.keys());
  const errors = [];
  const missingRust = difference(startupSet, rustSet);
  const unregisteredRust = difference(rustSet, startupSet);
  const unregisteredTs = difference(tsSet, startupSet);
  if (missingRust.length) errors.push(`startup 已注册但 Rust 未实现: ${missingRust.join(", ")}`);
  if (unregisteredRust.length) errors.push(`Rust command 未注册 startup: ${unregisteredRust.join(", ")}`);
  if (unregisteredTs.length) errors.push(`TS wrapper 未注册 startup: ${unregisteredTs.join(", ")}`);
  if (errors.length) throw new Error(errors.join("\n"));

  const generated = render(startup, rust, ts);
  const checking = process.argv.includes("--check");
  if (checking) {
    if (!fs.existsSync(outputPath)) throw new Error(`生成文件不存在: ${path.relative(root, outputPath)}`);
    if (fs.readFileSync(outputPath, "utf8") !== generated) {
      throw new Error("生成文件已漂移，请运行 node scripts/gen-command-docs.mjs");
    }
  } else {
    fs.mkdirSync(path.dirname(outputPath), { recursive: true });
    fs.writeFileSync(outputPath, generated);
  }
  process.stdout.write(`${checking ? "command 文档检查通过" : `已生成 ${path.relative(root, outputPath)}`}（${startup.size} 个 command）\n`);
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
}

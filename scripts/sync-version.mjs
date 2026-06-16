#!/usr/bin/env node
// 版本单一源同步器。
// 唯一可信源 = 根目录 `.version`（单行，如 `0.1.0`）。
// 默认：把 .version 写入所有 manifest。`--check`：仅校验一致性（CI 用），drift 则 exit 1。
//
// 用法:
//   node scripts/sync-version.mjs          # 写入各 manifest
//   node scripts/sync-version.mjs --check  # 校验，不一致 exit 1

import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const CHECK = process.argv.includes("--check");

/** 读 .version（trim，校验 semver 形态）。 */
function readVersion() {
  const raw = readFileSync(join(ROOT, ".version"), "utf8").trim();
  if (!/^\d+\.\d+\.\d+(-[\w.]+)?$/.test(raw)) {
    console.error(`[sync-version] .version 内容非法: "${raw}"（期望 semver 如 0.1.0）`);
    process.exit(1);
  }
  return raw;
}

/** JSON manifest：读取/替换顶层 version 字段（保留原缩进 2 空格）。 */
function jsonTarget(relPath) {
  return {
    path: relPath,
    read() {
      const obj = JSON.parse(readFileSync(join(ROOT, relPath), "utf8"));
      return obj.version;
    },
    write(version) {
      const full = join(ROOT, relPath);
      const obj = JSON.parse(readFileSync(full, "utf8"));
      obj.version = version;
      writeFileSync(full, JSON.stringify(obj, null, 2) + "\n");
    },
  };
}

/** Cargo.toml：仅替换 [package] 段首个 `version = "..."`（避免误伤依赖版本）。 */
function cargoTarget(relPath) {
  const VER_RE = /^version\s*=\s*"[^"]*"/m;
  return {
    path: relPath,
    read() {
      const text = readFileSync(join(ROOT, relPath), "utf8");
      const m = text.match(VER_RE);
      return m ? m[0].match(/"([^"]*)"/)[1] : null;
    },
    write(version) {
      const full = join(ROOT, relPath);
      const text = readFileSync(full, "utf8");
      writeFileSync(full, text.replace(VER_RE, `version = "${version}"`));
    },
  };
}

const targets = [
  jsonTarget("package.json"),
  jsonTarget("src-tauri/tauri.conf.json"),
  jsonTarget("docs/package.json"),
  cargoTarget("src-tauri/Cargo.toml"),
];

const version = readVersion();

if (CHECK) {
  const drift = targets.filter((t) => t.read() !== version);
  if (drift.length > 0) {
    console.error(`[sync-version] 版本漂移（期望 ${version}）:`);
    for (const t of drift) console.error(`  - ${t.path}: ${t.read()}`);
    console.error("修复: 运行 `node scripts/sync-version.mjs` 后提交。");
    process.exit(1);
  }
  console.log(`[sync-version] ✓ 所有 manifest 与 .version (${version}) 一致`);
} else {
  for (const t of targets) t.write(version);
  console.log(`[sync-version] ✓ 已同步 ${version} → ${targets.map((t) => t.path).join(", ")}`);
}

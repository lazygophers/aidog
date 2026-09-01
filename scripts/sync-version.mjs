#!/usr/bin/env node
// 版本单一源同步器。
// 唯一可信源 = 根目录 `.version`（单行，如 `0.1.0`）。
// 默认：把 .version 写入所有版本文件。`--bump`：自动 patch bump 后同步。`--set`：指定版本后同步。`--check`：仅校验一致性（CI 用），drift 则 exit 1。
//
// 用法:
//   node scripts/sync-version.mjs               # 写入所有版本文件
//   node scripts/sync-version.mjs --bump        # .version patch +1 后同步
//   node scripts/sync-version.mjs --set 0.1.13  # 指定 .version 后同步
//   node scripts/sync-version.mjs --check       # 校验，不一致 exit 1

import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const CHECK = process.argv.includes("--check");
const BUMP = process.argv.includes("--bump");
const SET_INDEX = process.argv.indexOf("--set");
const SET_VERSION = SET_INDEX === -1 ? null : process.argv[SET_INDEX + 1];
const SEMVER_RE = /^\d+\.\d+\.\d+(-[\w.]+)?$/;

if ([CHECK, BUMP, SET_VERSION !== null].filter(Boolean).length > 1) {
  console.error("[sync-version] --check / --bump / --set 只能选一个");
  process.exit(1);
}

if (SET_INDEX !== -1 && !SET_VERSION) {
  console.error("[sync-version] --set 需要版本号，如 0.1.13");
  process.exit(1);
}

function validateVersion(version, label = "version 内容非法") {
  if (!SEMVER_RE.test(version)) {
    console.error(`[sync-version] ${label}: "${version}"（期望 semver 如 0.1.0）`);
    process.exit(1);
  }
}

function readVersion() {
  const raw = readFileSync(join(ROOT, ".version"), "utf8").trim();
  validateVersion(raw, ".version 内容非法");
  return raw;
}

function bumpPatch(version) {
  validateVersion(version);
  const [core] = version.split("-");
  const [major, minor, patch] = core.split(".").map(Number);
  return `${major}.${minor}.${patch + 1}`;
}

function writeVersion(version) {
  validateVersion(version);
  writeFileSync(join(ROOT, ".version"), `${version}\n`);
}

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

function cargoLockTarget(relPath) {
  return {
    path: relPath,
    read() {
      const text = readFileSync(join(ROOT, relPath), "utf8");
      const versions = aidogCargoLockVersions(text);
      return versions.size === 1 ? [...versions][0] : null;
    },
    write(version) {
      const full = join(ROOT, relPath);
      const text = readFileSync(full, "utf8");
      writeFileSync(full, updateAidogCargoLockVersions(text, version));
    },
  };
}

function aidogCargoLockVersions(text) {
  const versions = new Set();
  for (const block of text.split(/\n(?=\[\[package\]\]\n)/)) {
    const name = block.match(/^name = "([^"]+)"$/m)?.[1];
    const version = block.match(/^version = "([^"]+)"$/m)?.[1];
    if (name && version && (name === "aidog" || name.startsWith("aidog_"))) versions.add(version);
  }
  return versions;
}

function updateAidogCargoLockVersions(text, version) {
  return text
    .split(/\n(?=\[\[package\]\]\n)/)
    .map((block) => {
      const name = block.match(/^name = "([^"]+)"$/m)?.[1];
      if (name !== "aidog" && !name?.startsWith("aidog_")) return block;
      return block.replace(/^version = "[^"]+"$/m, `version = "${version}"`);
    })
    .join("\n");
}

const targets = [
  jsonTarget("package.json"),
  jsonTarget("src-tauri/tauri.conf.json"),
  jsonTarget("docs/package.json"),
  cargoTarget("src-tauri/Cargo.toml"),
  cargoLockTarget("src-tauri/Cargo.lock"),
];

const version = BUMP ? bumpPatch(readVersion()) : SET_VERSION ?? readVersion();
if (SET_VERSION) validateVersion(SET_VERSION);

if (BUMP || SET_VERSION) writeVersion(version);

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

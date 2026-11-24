#!/usr/bin/env node
// 孤儿测试机械门禁：workspace 内所有 test_*.rs 必须在所属 crate 的模块树中有 `mod <文件名主干>;` 声明，
// 缺声明 = 文件从未被编译执行（先例：生图线 test_mcp_http.rs 漏接线，8 个测试静默不运行）。
// 用法：node scripts/check-orphan-tests.mjs
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const sourceRoots = [join(root, "src-tauri", "crates"), join(root, "src-tauri", "src")];
const failures = [];

// 有意不接线的存量孤儿白名单（match 为路径片段）：新增条目必须附原因。
// 这两份是旧 middleware API（单 action/config）时代的 WIP 覆盖率测试，现行模型已改
// actions: Vec<ActionStep>（aidog_db/src/models/middleware.rs），接线前须按新 API 重写。
const ALLOWLIST = [
  { match: "aidog_middleware/src/inbound/test_inbound.rs", reason: "旧 API WIP 测试，待按 actions/ActionStep 重写" },
  { match: "aidog_middleware/src/outbound/test_outbound.rs", reason: "旧 API WIP 测试，待按 actions/ActionStep 重写" },
];

function walkRs(dir, files = []) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) walkRs(path, files);
    else if (entry.isFile() && entry.name.endsWith(".rs")) files.push(path);
  }
  return files;
}

for (const sourceRoot of sourceRoots) {
  let testFiles;
  try {
    testFiles = walkRs(sourceRoot).filter((p) => /^test_.*\.rs$/.test(p.slice(p.lastIndexOf("/") + 1)));
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
    continue;
  }
  // 每个 test 文件只在其所属 crate 的 src 内找声明（crates/<crate>/src/...）
  const crateSrcIndex = sourceRoot.endsWith("crates")
    ? testFiles.reduce((map, p) => {
        const crateSrc = join(p.split(`${sourceRoot}/`)[1].split("/src/")[0], "src");
        map[crateSrc] ??= walkRs(join(sourceRoot, crateSrc));
        return map;
      }, {})
    : {};
  for (const testFile of testFiles) {
    const fileName = testFile.slice(testFile.lastIndexOf("/") + 1);
    const stem = fileName.slice(0, -3);
    const escaped = stem.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    // 接线两种形态：`mod <文件名主干>;` 或 `#[path = ".../<文件名>"] mod <任意名>;`
    const wired = new RegExp(`(\\bmod\\s+${escaped}\\s*;)|(path\\s*=\\s*"[^"]*${escaped}\\.rs")`);
    const candidates = sourceRoot.endsWith("crates")
      ? crateSrcIndex[join(testFile.split(`${sourceRoot}/`)[1].split("/src/")[0], "src")]
      : walkRs(sourceRoot);
    const declared = candidates.some((path) => wired.test(readFileSync(path, "utf8")));
    const rel = relative(root, testFile);
    if (!declared && !ALLOWLIST.some((a) => rel.includes(a.match))) failures.push(rel);
  }
}

if (failures.length) {
  for (const file of failures) {
    console.error(`${file}: 孤儿测试；在所属 crate 的模块树中添加 mod 声明。`);
  }
  process.exit(1);
}
console.log("孤儿测试检查通过。");

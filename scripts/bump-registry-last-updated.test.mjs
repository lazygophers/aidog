import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

// index.json 的真实形态：顶层 last_updated 与 "platforms" 同一行（线上文件已是此形态）。
test("改一个数据文件 → 盖戳并推高 index.json 顶层 last_updated（含 last_updated 与下个键同行）", () => {
  const root = mkdtempSync(join(tmpdir(), "bump-"));
  try {
    const git = (...a) => execFileSync("git", ["-C", root, ...a], { stdio: "pipe" });
    const reg = join(root, "src-tauri/defaults/registry");
    mkdirSync(join(root, "scripts"), { recursive: true });
    mkdirSync(join(reg, "platforms/x"), { recursive: true });
    cpSync(join(import.meta.dirname, "bump-registry-last-updated.mjs"), join(root, "scripts/bump.mjs"));
    writeFileSync(join(root, "package.json"), '{"type":"module"}\n');
    writeFileSync(join(reg, "index.json"), '{\n  "last_updated": 1,  "platforms": [\n    {\n      "code": "x"\n    }\n  ]\n}\n');
    writeFileSync(join(reg, "platforms/x/platform.json"), '{\n  "last_updated": 1,\n  "a": 1\n}\n');
    git("init", "-q");
    git("add", ".");
    git("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "init");
    writeFileSync(join(reg, "platforms/x/platform.json"), '{\n  "last_updated": 1,\n  "a": 2\n}\n');

    const r = spawnSync("node", [join(root, "scripts/bump.mjs")], { encoding: "utf8" });
    assert.equal(r.status, 0, r.stderr);
    assert.match(r.stdout, /1 个文件已盖戳/);
    const idx = JSON.parse(readFileSync(join(reg, "index.json"), "utf8"));
    assert.ok(idx.last_updated > 1, "index.json last_updated 未推高");
    assert.ok(JSON.parse(readFileSync(join(reg, "platforms/x/platform.json"), "utf8")).last_updated > 1);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

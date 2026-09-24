#!/usr/bin/env node
// Minimal test: verify bump-registry-last-updated handles nested untracked directories.
// Creates temp git repo with untracked nested registry structure, runs script, checks result.
import { execSync } from "node:child_process";
import { mkdirSync, writeFileSync, rmSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const tmp = join(tmpdir(), `test-bump-${Date.now()}`);
const reg = join(tmp, "src-tauri/defaults/registry");
const platforms = join(reg, "platforms/testplatform");

try {
  // Setup: temp git repo + tracked index.json + untracked nested dir with model JSON.
  mkdirSync(platforms, { recursive: true });
  writeFileSync(join(reg, "index.json"), `{\n  "version": 1\n}\n`);
  writeFileSync(join(platforms, "platform.json"), `{\n  "code": "test"\n}\n`);
  mkdirSync(join(platforms, "models"), { recursive: true });
  writeFileSync(join(platforms, "models/test-model.json"), `{\n  "model_id": "test"\n}\n`);

  execSync("git init -q && git add src-tauri/defaults/registry/index.json && git commit -qm init", { cwd: tmp });

  // Copy script into temp repo (script resolves ROOT via import.meta.dirname).
  const script = readFileSync(join(import.meta.dirname, "bump-registry-last-updated.mjs"), "utf8");
  mkdirSync(join(tmp, "scripts"), { recursive: true });
  writeFileSync(join(tmp, "scripts/bump-registry-last-updated.mjs"), script);

  // Run script (untracked nested dir should expand into JSON files).
  execSync("node scripts/bump-registry-last-updated.mjs", { cwd: tmp, stdio: "inherit", env: { ...process.env, BUMP_DEBUG: "1" } });

  // Verify: both nested model JSON and index.json got last_updated stamped.
  const model = JSON.parse(readFileSync(join(platforms, "models/test-model.json"), "utf8"));
  const index = JSON.parse(readFileSync(join(reg, "index.json"), "utf8"));
  if (!model.last_updated || !index.last_updated) {
    console.error("FAIL: last_updated missing");
    process.exit(1);
  }
  console.log("PASS: nested untracked directory expanded and stamped");
} finally {
  rmSync(tmp, { recursive: true, force: true });
}

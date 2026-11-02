// node --test scripts/check-registry-slots.test.mjs（yarn test:registry-slots）
// models.default / models.peak 槽位值必须是非空、元素非空、不重复的字符串数组，
// 且展平后 ⊆ model_list.default。喂坏输入 → check-registry 必须非零退出。
import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { cpSync, mkdtempSync, mkdirSync, readFileSync, readdirSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const realDir = join(here, "..", "src-tauri", "defaults", "registry");
const TARGET = "glm_coding"; // 唯一带 models.peak 的平台

/** 临时 registry：除 TARGET 的 platform.json 外全部软链真目录。 */
function run(mutate) {
  const dir = mkdtempSync(join(tmpdir(), "aidog-reg-"));
  try {
    symlinkSync(join(realDir, "schema"), join(dir, "schema"));
    cpSync(join(realDir, "index.json"), join(dir, "index.json"));
    mkdirSync(join(dir, "platforms", TARGET), { recursive: true });
    for (const c of readdirSync(join(realDir, "platforms"))) {
      if (c !== TARGET) symlinkSync(join(realDir, "platforms", c), join(dir, "platforms", c));
    }
    for (const f of readdirSync(join(realDir, "platforms", TARGET))) {
      if (f !== "platform.json") symlinkSync(join(realDir, "platforms", TARGET, f), join(dir, "platforms", TARGET, f));
    }
    const doc = JSON.parse(readFileSync(join(realDir, "platforms", TARGET, "platform.json"), "utf8"));
    mutate(doc);
    writeFileSync(join(dir, "platforms", TARGET, "platform.json"), JSON.stringify(doc, null, 2));
    const r = spawnSync("node", [join(here, "check-registry.mjs")], {
      env: { ...process.env, AIDOG_REGISTRY_DIR: dir },
      encoding: "utf8",
    });
    return { code: r.status, out: r.stdout + r.stderr };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

const first = (v) => (Array.isArray(v) ? v[0] : v);

test("基线（真 registry 内容）通过", () => {
  assert.equal(run(() => {}).code, 0);
});
test("旧字符串形态硬错", () => {
  assert.notEqual(run((d) => { d.models.default.sonnet = first(d.models.default.sonnet); }).code, 0);
});
test("peak 分支旧字符串形态硬错", () => {
  assert.notEqual(run((d) => { d.models.peak.default = first(d.models.peak.default); }).code, 0);
});
test("空数组硬错", () => {
  assert.notEqual(run((d) => { d.models.default.sonnet = []; }).code, 0);
});
test("空串元素硬错", () => {
  assert.notEqual(run((d) => { d.models.default.sonnet = [""]; }).code, 0);
});
test("重复元素硬错", () => {
  assert.notEqual(run((d) => { const v = first(d.models.default.sonnet); d.models.default.sonnet = [v, v]; }).code, 0);
});
test("槽位值不在 model_list.default 硬错", () => {
  const r = run((d) => { d.models.default.sonnet = [first(d.models.default.sonnet), "zz-not-in-list"]; });
  assert.notEqual(r.code, 0);
  assert.match(r.out, /model_list/);
});
test("peak 槽位值不在 model_list.default 硬错", () => {
  assert.notEqual(run((d) => { d.models.peak.default = ["zz-not-in-list"]; }).code, 0);
});

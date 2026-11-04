// node --test scripts/check-registry-ratelimit.test.mjs（yarn test:registry-ratelimit）
// platform.json 的 rate_limit_policies / rate_limit_error_codes：喂坏输入 → check-registry 必须
// 非零退出且报出对应原因（断言消息，不只看退出码，避免被无关错误蒙混）。
import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { cpSync, mkdtempSync, mkdirSync, readFileSync, readdirSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const realDir = join(here, "..", "src-tauri", "defaults", "registry");
const TARGET = "glm"; // 带 models 目录（glm-4-plus 存在）

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

const policy = (over = {}) => ({
  id: "t1",
  label: "T1",
  source_url: "https://docs.bigmodel.cn/cn/guide/platform/equity-explain.md",
  concurrency: 5,
  rpm: 60,
  tpm: 100000,
  models: { "glm-4-plus": { concurrency: 20 } },
  windows: [{ start_hour: 0, end_hour: 8, timezone: "Asia/Shanghai", days_of_week: [1, 2], concurrency: 10 }],
  ...over,
});
const withPolicies = (...ps) => (d) => { d.rate_limit_policies = ps; };
const bad = (mutate, re) => {
  const r = run(mutate);
  assert.notEqual(r.code, 0, r.out);
  assert.match(r.out, re);
};

test("合法策略（全字段）与骨架策略（仅 id/label/source_url）通过", () => {
  const r = run(withPolicies(policy(), { id: "skeleton", label: "S", source_url: "https://docs.z.ai/x" }));
  assert.equal(r.code, 0, r.out);
});
test("缺 source_url 硬错", () => {
  const p = policy(); delete p.source_url;
  bad(withPolicies(p), /source_url/);
});
test("数字非正整数硬错（0 / 负数 / 小数 / 字符串）", () => {
  for (const v of [0, -1, 1.5, "5"]) bad(withPolicies(policy({ concurrency: v })), /rate_limit_policies.*concurrency/);
});
test("策略 id 重复硬错", () => {
  bad(withPolicies(policy(), policy()), /id 重复/);
});
test("windows 小时越界硬错", () => {
  bad(withPolicies(policy({ windows: [{ start_hour: 24, end_hour: 8 }] })), /start_hour/);
  bad(withPolicies(policy({ windows: [{ start_hour: 0, end_hour: 25 }] })), /end_hour/);
  bad(withPolicies(policy({ windows: [{ start_hour: 0, end_hour: -1 }] })), /end_hour/);
});
test("windows end_hour=24（当日结束）合法", () => {
  const r = run(withPolicies(policy({ windows: [{ start_hour: 0, end_hour: 24 }] })));
  assert.equal(r.code, 0, r.out);
});
test("windows days_of_week 越界硬错", () => {
  bad(withPolicies(policy({ windows: [{ start_hour: 0, end_hour: 8, days_of_week: [7] }] })), /days_of_week/);
});
test("windows timezone 非法 IANA 硬错", () => {
  bad(withPolicies(policy({ windows: [{ start_hour: 0, end_hour: 8, timezone: "Mars/Base" }] })), /timezone/);
});
test("模型级键不在 models 目录硬错", () => {
  bad(withPolicies(policy({ models: { "zz-not-a-model": { concurrency: 1 } } })), /不在 models 目录/);
});
test("error_codes 值不在词表硬错", () => {
  bad((d) => { d.rate_limit_error_codes = { "1302": "banana" }; }, /rate_limit_error_codes/);
});
test("error_codes 键非纯数字硬错", () => {
  bad((d) => { d.rate_limit_error_codes = { "13x2": "rate_limit" }; }, /rate_limit_error_codes/);
});
test("error_codes 合法值通过", () => {
  const r = run((d) => { d.rate_limit_error_codes = { "1302": "rate_limit", "1305": "overloaded" }; });
  assert.equal(r.code, 0, r.out);
});

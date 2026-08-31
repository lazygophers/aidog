#!/usr/bin/env node
// registry JSON Schema 校验：index.json + platforms/*/platform.json + platforms/*/models/**/*.json
// 逐文件对照 src-tauri/defaults/registry/schema/ 的三个 schema（draft-07）。
// 另检查 models 目录/文件缺失（见下方「models 完整性检查」）。
// 用法：node scripts/check-registry.mjs   （AIDOG_REGISTRY_DIR 可指 fixture 目录；AIDOG_REGISTRY_STRICT=1 开严格模式）
// 与 Rust 侧 test_registry.rs 的漂移断言互补：那边锁清单一致性，这边锁字段形状。

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import Ajv from "ajv";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const registryDir = process.env.AIDOG_REGISTRY_DIR ?? join(scriptDir, "..", "src-tauri", "defaults", "registry");
const schemaDir = join(registryDir, "schema");

const ajv = new Ajv({ allErrors: true, strict: false });
const schemas = {
  index: ajv.compile(JSON.parse(readFileSync(join(schemaDir, "index.schema.json"), "utf8"))),
  platform: ajv.compile(JSON.parse(readFileSync(join(schemaDir, "platform.schema.json"), "utf8"))),
  model: ajv.compile(JSON.parse(readFileSync(join(schemaDir, "model.schema.json"), "utf8"))),
};

const failures = [];
const warnings = [];
let checked = 0;

function validate(kind, rel, raw) {
  let doc;
  try {
    doc = JSON.parse(raw);
  } catch (e) {
    failures.push([rel, `JSON 解析失败: ${e.message}`]);
    return;
  }
  checked++;
  if (!schemas[kind](doc)) {
    for (const err of schemas[kind].errors) {
      failures.push([rel, `${err.instancePath || "/"} ${err.message}`]);
    }
  }
}

// out = 相对路径（index.json 登记用），ids = 文件内 model_id（platform.json 引用用）。
// 两者通常相同；macOS 文件系统不分大小写，装不下只差大小写的两个 id（如 atlascloud
// `Qwen/...` 与 `qwen/...`），这种条目路径退化成小写、真值仍是文件内的 model_id。
function walkModels(dir, baseRel, rel, out, ids) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) {
      walkModels(p, baseRel, rel === "" ? name : `${rel}/${name}`, out, ids);
    } else if (name.endsWith(".json")) {
      const raw = readFileSync(p, "utf8");
      validate("model", `${baseRel}/${rel === "" ? name : `${rel}/${name}`}`, raw);
      out.push(rel === "" ? name.replace(/\.json$/, "") : `${rel}/${name.replace(/\.json$/, "")}`);
      if (ids) {
        try {
          ids.push(JSON.parse(raw).model_id);
        } catch {
          /* schema 校验已经报过错，这里不重复 */
        }
      }
    }
  }
}

validate("index", "index.json", readFileSync(join(registryDir, "index.json"), "utf8"));

// models 完整性检查（make lint 门禁）：
// ① index.json 声明的 models[] 与磁盘零差集：声明了文件但目录/文件缺失、磁盘多出未登记文件，均为错。
// ② platform.json 引用的 model id（models.* 分支值 + model_list.* 条目）必须有对应 models/<id>.json：
//    平台已带 models 目录（自建价目）时为错；完全没目录的中转平台仅 warning
//    （AIDOG_REGISTRY_STRICT=1 把 warning 也升级为错）。
const strict = process.env.AIDOG_REGISTRY_STRICT === "1";
// 无 models 目录豁免：协议/订阅/自部署类平台，计费不走 per-token 价目
// （用户决策 2026-08-27：除这些外所有引用了 model id 的平台都必须有 models 目录）。
// 豁免平台缺文件仍报 warning，strict 模式升级为错。
const EXEMPT_NO_MODELS = new Set([
  "mock", // 假协议
  "claude_code", "codex", "devin", // 订阅/虚拟映射，无 per-token 价
  "newapi", // 自部署中转，价目随部署方
  "kimi_coding", "qianfan_coding", "xiaomi_mimo_coding", "xiaomi_mimo_coding_en", "bailian_coding",
  "bailian_coding_en",
  "compshare_coding", "minimax_coding", "glm_coding_en", // coding 订阅套餐
]);
const indexDoc = JSON.parse(readFileSync(join(registryDir, "index.json"), "utf8"));

// ③ 目录注册完整性（2026-08-28 补洞；2026-08-31 收紧）：
// platforms/ 下每个目录必须被 index.json 的 platforms 或 pricing_only 之一登记；
// platforms 条目必须有 platform.json（缺了此前被静默 continue 跳过，lint 漏检）；
// pricing_only 是纯协议豁免清单（无 platform.json），禁止登记非纯协议平台——
// 2026-08-31 用户决策：litellm/mistral 已升级正式平台，清单现应为空。
const pricingOnly = new Set((indexDoc.pricing_only ?? []).map((e) => e.code));
const registered = new Set([...indexDoc.platforms.map((e) => e.code), ...pricingOnly]);
for (const name of readdirSync(join(registryDir, "platforms"))) {
  if (name === ".DS_Store") continue;
  if (!registered.has(name)) failures.push([`platforms/${name}`, "目录未登记进 index.json（platforms / pricing_only 都没有）"]);
}
for (const code of pricingOnly) {
  if (statSync(join(registryDir, "platforms", code, "platform.json"), { throwIfNoEntry: false })) {
    failures.push([`index.json[pricing_only:${code}]`, "pricing_only 条目不应有 platform.json"]);
  }
}

for (const entry of indexDoc.platforms) {
  const code = entry.code;
  const modelsDir = join(registryDir, "platforms", code, "models");
  const disk = [];
  const diskIds = [];
  if (statSync(modelsDir, { throwIfNoEntry: false })?.isDirectory()) {
    walkModels(modelsDir, `platforms/${code}/models`, "", disk, diskIds);
  }
  const diskSet = new Set(disk);
  const idSet = new Set(diskIds);
  const declared = new Set((entry.models ?? []).map((f) => f.replace(/\.json$/, "")));

  if (declared.size > 0 && diskSet.size === 0) {
    failures.push([`index.json[${code}]`, `声明了 ${declared.size} 个 models 文件但目录 ${entry.models_dir} 不存在`]);
  }
  for (const f of declared) {
    if (!diskSet.has(f)) failures.push([`index.json[${code}]`, `声明的 ${f}.json 磁盘上不存在`]);
  }
  for (const f of diskSet) {
    if (!declared.has(f)) failures.push([`platforms/${code}/models`, `${f}.json 未登记进 index.json`]);
  }

  const platformPath = join(registryDir, "platforms", code, "platform.json");
  if (!statSync(platformPath, { throwIfNoEntry: false })) {
    failures.push([`platforms/${code}`, "platform.json 缺失（index.json platforms 条目必须带 platform.json）"]);
    continue;
  }
  const platformDoc = JSON.parse(readFileSync(platformPath, "utf8"));
  const refs = new Set();
  for (const branch of Object.values(platformDoc.models ?? {})) {
    for (const id of Object.values(branch ?? {})) refs.add(id);
  }
  for (const branch of Object.values(platformDoc.model_list ?? {})) {
    for (const id of branch ?? []) refs.add(id);
  }
  const missing = [...refs].filter((id) => !idSet.has(id));
  if (missing.length) {
    const msg = `引用 ${missing.length} 个 model id 无对应文件: ${missing.slice(0, 5).join(", ")}${missing.length > 5 ? " …" : ""}`;
    // 豁免平台且整个平台没有 models 目录 → warning；有目录缺文件、或非豁免平台缺目录 → 硬错
    if (EXEMPT_NO_MODELS.has(code) && diskSet.size === 0 && !strict) warnings.push(`platforms/${code}: ${msg}`);
    else failures.push([`platforms/${code}/platform.json`, msg]);
  }
}

// ④ pricing_only 条目（纯协议豁免，现应为空）：模型文件 schema 校验 + 盘上/声明零差集。
// 2026-08-28 前这批文件完全绕过 models 完整性检查。
for (const entry of indexDoc.pricing_only ?? []) {
  const code = entry.code;
  const modelsDir = join(registryDir, "platforms", code, "models");
  const disk = [];
  if (statSync(modelsDir, { throwIfNoEntry: false })?.isDirectory()) {
    walkModels(modelsDir, `platforms/${code}/models`, "", disk);
  }
  const diskSet = new Set(disk);
  const declared = new Set((entry.models ?? []).map((f) => f.replace(/\.json$/, "")));
  for (const f of declared) {
    if (!diskSet.has(f)) failures.push([`index.json[pricing_only:${code}]`, `声明的 ${f}.json 磁盘上不存在`]);
  }
  for (const f of diskSet) {
    if (!declared.has(f)) failures.push([`platforms/${code}/models`, `${f}.json 未登记进 index.json pricing_only 清单`]);
  }
}

// ⑤ schema 自检：schema/ 三个文件的每个字段（properties 下每个子 schema）都必须带 description，
// 且首 token 必须是【必填】/【可选】，与所在对象的 required 数组一致（票 #18）。
// schema 是 registry 的字段说明书（CLAUDE.md 引用），漏 description 或前缀错标对新维护者是黑盒。
// walk 递归嵌套 object properties + items + additionalProperties（旧版漏嵌套 object，endpoints.default
// 等字段的 description 检查从未生效——2026-08-29 票 #18 修复）。
const REQUIRED_PREFIX = "【必填】";
const OPTIONAL_PREFIX = "【可选】";
const PREFIX_RE = /^(【必填】|【可选】)/;
for (const f of ["index.schema.json", "platform.schema.json", "model.schema.json"]) {
  const doc = JSON.parse(readFileSync(join(schemaDir, f), "utf8"));
  const miss = [];
  const walk = (node, path) => {
    if (!node || typeof node !== "object" || Array.isArray(node)) return;
    const props = node.properties;
    if (props) {
      const req = new Set(node.required || []);
      for (const [k, v] of Object.entries(props)) {
        if (v && typeof v === "object") {
          if (v.description === undefined) miss.push([path + k, "缺 description（每个字段必须存在 description）"]);
          else if (typeof v.description === "string") {
            const want = req.has(k) ? REQUIRED_PREFIX : OPTIONAL_PREFIX;
            if (!v.description.startsWith(want)) {
              miss.push([path + k, `description 前缀应为 ${want}，实际是 ${v.description.slice(0, 12)}`]);
            }
          }
        }
        walk(v, `${path}${k}.`);
      }
    }
    walk(node.items, `${path}items.`);
    walk(node.additionalProperties, `${path}ap.`);
  };
  walk(doc, "");
  for (const [m, msg] of miss) failures.push([`schema/${f}`, `字段 ${m} ${msg}`]);
}

if (failures.length) {
  console.error(`registry 校验失败：${failures.length} 处 / ${checked} 个文件`);
  for (const [file, msg] of failures) console.error(`  ${file}: ${msg}`);
  process.exit(1);
}
console.log(`registry schema 校验通过：${checked} 个文件（1 index + platforms + models）`);
if (warnings.length) {
  console.log(`registry warning：${warnings.length} 个平台引用了无文件的 model id（中转平台常见，AIDOG_REGISTRY_STRICT=1 时报错）`);
  // 票 #19：逐平台明细（原先只汇总会数，43 平台谁缺多少条不可见）
  for (const w of warnings) console.log(`  ${w}`);
}

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

function walkModels(dir, baseRel, rel, out) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) {
      walkModels(p, baseRel, rel === "" ? name : `${rel}/${name}`, out);
    } else if (name.endsWith(".json")) {
      validate("model", `${baseRel}/${rel === "" ? name : `${rel}/${name}`}`, readFileSync(p, "utf8"));
      // 相对 models/ 的路径（含 vendor 子目录）去掉 .json = model id
      out.push(rel === "" ? name.replace(/\.json$/, "") : `${rel}/${name.replace(/\.json$/, "")}`);
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
const indexDoc = JSON.parse(readFileSync(join(registryDir, "index.json"), "utf8"));
for (const entry of indexDoc.platforms) {
  const code = entry.code;
  const modelsDir = join(registryDir, "platforms", code, "models");
  const disk = [];
  if (statSync(modelsDir, { throwIfNoEntry: false })?.isDirectory()) {
    walkModels(modelsDir, `platforms/${code}/models`, "", disk);
  }
  const diskSet = new Set(disk);
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
  if (!statSync(platformPath, { throwIfNoEntry: false })) continue;
  const platformDoc = JSON.parse(readFileSync(platformPath, "utf8"));
  const refs = new Set();
  for (const branch of Object.values(platformDoc.models ?? {})) {
    for (const id of Object.values(branch ?? {})) refs.add(id);
  }
  for (const branch of Object.values(platformDoc.model_list ?? {})) {
    for (const id of branch ?? []) refs.add(id);
  }
  const missing = [...refs].filter((id) => !diskSet.has(id));
  if (missing.length) {
    const msg = `引用 ${missing.length} 个 model id 无对应文件: ${missing.slice(0, 5).join(", ")}${missing.length > 5 ? " …" : ""}`;
    if (diskSet.size > 0 || strict) failures.push([`platforms/${code}/platform.json`, msg]);
    else warnings.push(`platforms/${code}: ${msg}`);
  }
}

if (failures.length) {
  console.error(`registry 校验失败：${failures.length} 处 / ${checked} 个文件`);
  for (const [file, msg] of failures) console.error(`  ${file}: ${msg}`);
  process.exit(1);
}
console.log(`registry schema 校验通过：${checked} 个文件（1 index + platforms + models）`);
if (warnings.length) {
  console.log(`registry warning：${warnings.length} 个平台引用了无文件的 model id（中转平台常见，AIDOG_REGISTRY_STRICT=1 时报错）`);
}

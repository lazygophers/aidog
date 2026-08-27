#!/usr/bin/env node
// registry JSON Schema 校验：index.json + platforms/*/platform.json + platforms/*/models/**/*.json
// 逐文件对照 src-tauri/defaults/registry/schema/ 的三个 schema（draft-07）。
// 用法：node scripts/check-registry.mjs   （AIDOG_REGISTRY_DIR 可指到 fixture 目录）
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

validate("index", "index.json", readFileSync(join(registryDir, "index.json"), "utf8"));

function walkModels(dir, base) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) {
      walkModels(p, `${base}/${name}`);
    } else if (name.endsWith(".json")) {
      validate("model", `${base}/${name}`, readFileSync(p, "utf8"));
    }
  }
}

for (const code of readdirSync(join(registryDir, "platforms"))) {
  const dir = join(registryDir, "platforms", code);
  if (!statSync(dir).isDirectory()) continue;
  const platformJson = join(dir, "platform.json");
  if (statSync(platformJson, { throwIfNoEntry: false })) {
    validate("platform", `platforms/${code}/platform.json`, readFileSync(platformJson, "utf8"));
  }
  const modelsDir = join(dir, "models");
  if (statSync(modelsDir, { throwIfNoEntry: false })?.isDirectory()) {
    walkModels(modelsDir, `platforms/${code}/models`);
  }
}

if (failures.length) {
  console.error(`registry schema 校验失败：${failures.length} 处 / ${checked} 个文件`);
  for (const [file, msg] of failures) console.error(`  ${file}: ${msg}`);
  process.exit(1);
}
console.log(`registry schema 校验通过：${checked} 个文件（1 index + platforms + models）`);

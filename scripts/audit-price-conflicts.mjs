#!/usr/bin/env node
// issue #165：聚合平台价格与 official:true 官方条目的冲突审计。
// 按 canonical_model 跨平台分组，组内同时存在 official:true 条目与非官方条目时，
// 逐字段比较价格，输出可复现冲突清单（机器可读 --json <path>，人读 stdout 摘要）。
// 只产清单，不改任何数据。
//
// 冲突口径（写死，勿改语义）：
// - 比较范围 = 组内每个非官方条目 vs 组内任一 official:true 条目（多个官方条目时取
//   canonical_model 相同的全部官方条目逐一比较，任一一致即视为「与官方一致」不冲突）。
// - 只比较顶层平价字段：price.input / price.output / price.cache_read /
//   price.cache_write / price.cache_write_1h。两侧均存在且为数值的才进比较，
//   单侧缺失的字段跳过（不算冲突，也不算不可比）。
// - 不可比（单独归类 NONCOMPARABLE，不计入冲突数）：
//   * 任一侧 price.free === true（显式免费标记，0 价有意为之）；
//   * 任一侧 price.unit 存在且 ≠ "token"（按次 / 按量计价，口径不同）；
//   * 任一侧条目含 context_tiers 或 time_tiers（分档价，平价比较无效）。
// - price.peak（高峰绝对价）：两侧均有才比较，差异单独记 peak 字段冲突；
//   单侧有不算冲突（peak 是可选增强价，缺省侧回落平价链）。
// - currency 不一致 → 冲突，类别 currency。
// - 数值比较要求精确相等（无容差）：registry 存精确值，聚合平台上浮（如 +10%）
//   也算冲突，ratio 字段暴露倍差供人工判断是否为有意上浮。
// - 聚合键 = 条目显式 canonical_model（缺省回落 model_id）。

import { readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join, dirname, relative } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const registryDir =
  process.env.AIDOG_REGISTRY_DIR ?? join(scriptDir, "..", "src-tauri", "defaults", "registry");

const FLAT_FIELDS = ["input", "output", "cache_read", "cache_write", "cache_write_1h"];

function walkModels(dir, platform, out = []) {
  let entries;
  try {
    entries = readdirSync(dir, { withFileTypes: true });
  } catch {
    return out;
  }
  for (const e of entries) {
    const p = join(dir, e.name);
    if (e.isDirectory()) walkModels(p, platform, out);
    else if (e.name.endsWith(".json")) out.push({ platform, file: p });
  }
  return out;
}

const platformsDir = join(registryDir, "platforms");
const entries = [];
for (const code of readdirSync(platformsDir)) {
  try {
    if (!statSync(join(platformsDir, code)).isDirectory()) continue;
  } catch {
    continue;
  }
  walkModels(join(platformsDir, code, "models"), code, entries);
}

const byCanonical = new Map();
for (const { platform, file } of entries) {
  let doc;
  try {
    doc = JSON.parse(readFileSync(file, "utf8"));
  } catch {
    continue;
  }
  const canonical = doc.canonical_model ?? doc.model_id;
  if (!canonical || !doc.price) continue;
  if (!byCanonical.has(canonical)) byCanonical.set(canonical, []);
  byCanonical.get(canonical).push({ platform, file: relative(registryDir, file), doc });
}

const conflicts = [];
const noncomparable = [];
let officialGroups = 0;

for (const [canonical, group] of byCanonical) {
  const officials = group.filter((g) => g.doc.official === true);
  const others = group.filter((g) => g.doc.official !== true);
  if (officials.length === 0 || others.length === 0) continue;
  officialGroups++;

  for (const o of others) {
    const a = o.doc.price;
    const verdicts = officials.map((of_) => {
      const b = of_.doc.price;
      // 不可比判定
      if (a.free === true || b.free === true) return "noncomparable:free";
      const au = a.unit ?? "token";
      const bu = b.unit ?? "token";
      if (au !== "token" || bu !== "token") return "noncomparable:unit";
      if (of_.doc.price.context_tiers || of_.doc.price.time_tiers) return "noncomparable:tiers";
      if (o.doc.price.context_tiers || o.doc.price.time_tiers) return "noncomparable:tiers";
      // currency
      if ((a.currency ?? "USD") !== (b.currency ?? "USD")) return "conflict:currency";
      // 平价字段
      for (const f of FLAT_FIELDS) {
        const av = a[f];
        const bv = b[f];
        if (typeof av === "number" && typeof bv === "number" && av !== bv) return "conflict";
      }
      // peak 绝对价
      if (a.peak && b.peak) {
        for (const f of FLAT_FIELDS) {
          const av = a.peak[f];
          const bv = b.peak[f];
          if (typeof av === "number" && typeof bv === "number" && av !== bv)
            return "conflict:peak";
        }
      }
      return "match";
    });

    if (verdicts.some((v) => v === "match")) continue; // 与任一官方条目一致
    const isNoncomparable = verdicts.every((v) => v.startsWith("noncomparable"));
    const rec = {
      canonical_model: canonical,
      platform: o.platform,
      file: o.file,
      entry_price: pickPrice(a),
      official_refs: officials.map((x) => ({ platform: x.platform, file: x.file, price: pickPrice(x.doc.price) })),
    };
    if (isNoncomparable) {
      rec.reason = verdicts[0].split(":")[1];
      noncomparable.push(rec);
    } else {
      // 差异字段明细（对第一个官方条目）
      const b = officials[0].doc.price;
      const diffs = [];
      if ((a.currency ?? "USD") !== (b.currency ?? "USD")) diffs.push("currency");
      for (const f of FLAT_FIELDS) {
        const av = a[f];
        const bv = b[f];
        if (typeof av === "number" && typeof bv === "number" && av !== bv)
          diffs.push({ field: f, aggregator: av, official: bv, ratio: +(av / bv).toFixed(4) });
      }
      rec.diff_fields = diffs;
      conflicts.push(rec);
    }
  }
}

conflicts.sort((x, y) => x.platform.localeCompare(y.platform) || x.canonical_model.localeCompare(y.canonical_model));
noncomparable.sort((x, y) => x.platform.localeCompare(y.platform) || x.canonical_model.localeCompare(y.canonical_model));

function pickPrice(p) {
  const out = {};
  for (const f of [...FLAT_FIELDS, "unit", "free", "currency"]) if (p[f] !== undefined) out[f] = p[f];
  return out;
}

if (process.argv[2] === "--json" && process.argv[3]) {
  writeFileSync(process.argv[3], JSON.stringify({ conflicts, noncomparable, officialGroups }, null, 2) + "\n");
  console.log(`written: ${process.argv[3]}`);
}

console.log(`canonical groups with official entries: ${officialGroups}`);
console.log(`conflicts: ${conflicts.length}`);
const byPlat = {};
for (const c of conflicts) byPlat[c.platform] = (byPlat[c.platform] ?? 0) + 1;
console.log(`  by platform: ${JSON.stringify(byPlat)}`);
console.log(`noncomparable (free/unit/tiers): ${noncomparable.length}`);
const byReason = {};
for (const c of noncomparable) byReason[c.reason] = (byReason[c.reason] ?? 0) + 1;
console.log(`  by reason: ${JSON.stringify(byReason)}`);
if (!process.argv[2]) {
  for (const c of conflicts.slice(0, 40)) {
    console.log(`  ${c.platform}  ${c.canonical_model}  ${JSON.stringify(c.diff_fields)}`);
  }
  if (conflicts.length > 40) console.log(`  ... and ${conflicts.length - 40} more (use --json)`);
}
if (process.env.AIDOG_CONFLICT_EXIT_ON_CONFLICTS === "1" && conflicts.length > 0) process.exit(1);

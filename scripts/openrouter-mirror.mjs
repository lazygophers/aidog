#!/usr/bin/env node
import fs from "fs";
import path from "path";

const snapshotPath = "/tmp/openrouter_official.json";
const registryPath = "/Users/luoxin/persons/lyxamour/aidog/.claude/worktrees/agent-a6901a9c712a338c9/src-tauri/defaults/registry/platforms/openrouter";

// Load snapshot
const snapshot = JSON.parse(fs.readFileSync(snapshotPath, "utf-8"));
const models = snapshot.data || [];

// Load existing models directory
const modelsDir = path.join(registryPath, "models");
const existingModels = new Set();
if (fs.existsSync(modelsDir)) {
  fs.readdirSync(modelsDir).forEach(f => {
    if (f.endsWith(".json")) {
      const name = f.slice(0, -5);
      existingModels.add(name);
    }
  });
}

// Filter: active/current models with context_window + pricing
const filtered = models.filter(m => {
  const isActive = !m.id?.includes(":deprecated") && !m.description?.includes("deprecated");
  const hasContext = m.context_length > 0;
  const hasPrice = m.pricing && (m.pricing.prompt || m.pricing.completion);
  return isActive && hasContext && hasPrice;
});

console.log(`Snapshot: ${models.length} total`);
console.log(`Filtered: ${filtered.length} active with context + pricing`);
console.log(`Existing: ${existingModels.size}`);

// Plan new entries
const newEntries = [];
filtered.forEach(m => {
  const slug = m.id.replace(/\//g, "_");
  if (existingModels.has(slug)) return;

  const model_id = m.id;
  const context_window = m.context_length || null;

  const price = {};
  if (m.pricing) {
    const promptPrice = parseFloat(m.pricing.prompt) || 0;
    const completionPrice = parseFloat(m.pricing.completion) || 0;
    if (promptPrice > 0) price.input = promptPrice;
    if (completionPrice > 0) price.output = completionPrice;
    if (m.pricing.input_cache_read) {
      price.cache_read = parseFloat(m.pricing.input_cache_read);
    }
    if (m.pricing.input_cache_write) {
      price.cache_write = parseFloat(m.pricing.input_cache_write);
    }
  }

  const entry = {
    model_id,
    display_name: m.name || m.id,
    context_window,
    official: false,
    capabilities: {}, // OpenRouter doesn't document capabilities, use empty
    price
  };

  if (m.canonical_slug) {
    entry.canonical_model = m.canonical_slug;
  }

  newEntries.push({ slug, entry });
});

console.log(`\nNew entries: ${newEntries.length}`);

// Write new entries
if (!fs.existsSync(modelsDir)) {
  fs.mkdirSync(modelsDir, { recursive: true });
}

const now = Math.floor(Date.now() / 1000);
newEntries.forEach(({ slug, entry }) => {
  const outPath = path.join(modelsDir, `${slug}.json`);
  const output = { ...entry, last_updated: now };
  fs.writeFileSync(outPath, JSON.stringify(output, null, 2) + "\n");
  console.log(`  + ${slug}.json`);
});

// Update platform.json
const platformPath = path.join(registryPath, "platform.json");
const platform = JSON.parse(fs.readFileSync(platformPath, "utf-8"));
platform.last_updated = now;
fs.writeFileSync(platformPath, JSON.stringify(platform, null, 2) + "\n");
console.log(`\nUpdated platform.json timestamp: ${now}`);

// Update index.json
const indexPath = "/Users/luoxin/persons/lyxamour/aidog/.claude/worktrees/agent-a6901a9c712a338c9/src-tauri/defaults/registry/index.json";
const index = JSON.parse(fs.readFileSync(indexPath, "utf-8"));
index.last_updated = now;
fs.writeFileSync(indexPath, JSON.stringify(index, null, 2) + "\n");
console.log(`Updated index.json timestamp: ${now}`);

// Rebuild index.json openrouter models list
console.log(`\nRebuilding index.json openrouter entry...`);
const allFiles = fs.readdirSync(modelsDir).filter(f => f.endsWith('.json'));
const modelListEntries = allFiles.map(f => `openrouter/${f}`);

const openrouterEntryIdx = index.platforms.findIndex(p => p.code === 'openrouter');
if (openrouterEntryIdx >= 0) {
  index.platforms[openrouterEntryIdx].models = modelListEntries;
  console.log(`Updated openrouter index entry with ${modelListEntries.length} files`);
}

fs.writeFileSync(indexPath, JSON.stringify(index, null, 2) + "\n");
console.log(`Rewrote index.json with updated model list`);

console.log(`\n--- SUMMARY ---`);
console.log(`new_count=${newEntries.length}`);
console.log(`snapshot_total=${models.length}`);
console.log(`filtered_count=${filtered.length}`);
console.log(`existing_count=${existingModels.size}`);
console.log(`timestamp=${now}`);

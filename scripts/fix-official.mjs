#!/usr/bin/env node
import fs from "fs";
import path from "path";

// Find model_ids that only appear on openrouter and need official: true
const registryDir = "src-tauri/defaults/registry/platforms";

// Collect all model_ids across all platforms
const allModelIds = {}; // model_id -> [{platform, official}]

const platforms = fs.readdirSync(registryDir).filter(p => {
  return fs.statSync(path.join(registryDir, p)).isDirectory();
});

for (const platform of platforms) {
  const modelsDir = path.join(registryDir, platform, "models");
  if (!fs.existsSync(modelsDir)) continue;

  // Walk subdirs
  const walkDir = (dir) => {
    const entries = fs.readdirSync(dir);
    for (const entry of entries) {
      const fullPath = path.join(dir, entry);
      const stat = fs.statSync(fullPath);
      if (stat.isDirectory()) {
        walkDir(fullPath);
      } else if (entry.endsWith('.json')) {
        const data = JSON.parse(fs.readFileSync(fullPath, 'utf-8'));
        const id = data.model_id;
        if (id) {
          if (!allModelIds[id]) allModelIds[id] = [];
          allModelIds[id].push({ platform, official: data.official === true });
        }
      }
    }
  };
  walkDir(modelsDir);
}

// Find model_ids on openrouter with official: false that have no official entry elsewhere
const openrouterDir = path.join(registryDir, "openrouter", "models");
let fixed = 0;

const walkAndFix = (dir) => {
  const entries = fs.readdirSync(dir);
  for (const entry of entries) {
    const fullPath = path.join(dir, entry);
    const stat = fs.statSync(fullPath);
    if (stat.isDirectory()) {
      walkAndFix(fullPath);
    } else if (entry.endsWith('.json')) {
      const data = JSON.parse(fs.readFileSync(fullPath, 'utf-8'));
      if (data.official === false) {
        const id = data.model_id;
        const allEntries = allModelIds[id] || [];
        const hasOfficialElsewhere = allEntries.some(e => e.platform !== 'openrouter' && e.official);
        if (!hasOfficialElsewhere) {
          data.official = true;
          fs.writeFileSync(fullPath, JSON.stringify(data, null, 2) + "\n");
          fixed++;
        }
      }
    }
  }
};
walkAndFix(openrouterDir);

console.log(`Fixed official=true for ${fixed} entries with no official entry elsewhere`);

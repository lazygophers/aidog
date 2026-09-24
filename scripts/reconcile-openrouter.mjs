#!/usr/bin/env node
import fs from "fs";
import path from "path";

const modelsDir = "src-tauri/defaults/registry/platforms/openrouter/models";
const indexPath = "src-tauri/defaults/registry/index.json";

// Get all flat files (top-level *.json)
const flatFiles = fs.readdirSync(modelsDir).filter(f => f.endsWith('.json') && !f.includes('/'));

// Get all subdirectory files (subdir/*.json)
const subdirs = fs.readdirSync(modelsDir).filter(f => {
  const p = path.join(modelsDir, f);
  return fs.statSync(p).isDirectory();
});

const subdirFiles = new Set();
subdirs.forEach(d => {
  const dPath = path.join(modelsDir, d);
  fs.readdirSync(dPath).forEach(f => {
    if (f.endsWith('.json')) {
      subdirFiles.add(`${d}/${f}`);
    }
  });
});

console.log(`Flat files: ${flatFiles.length}, Subdirectory files: ${subdirFiles.size}`);

let deleted = 0;
let converted = 0;

// For each flat file, decide what to do
flatFiles.forEach(flatName => {
  const flatPath = path.join(modelsDir, flatName);
  const data = JSON.parse(fs.readFileSync(flatPath, 'utf-8'));
  const modelId = data.model_id; // e.g. "aion-labs/aion-2.0"

  if (!modelId) {
    console.log(`SKIP: ${flatName} has no model_id`);
    return;
  }

  // Construct the subdirectory path this model_id would map to
  const parts = modelId.split('/');
  if (parts.length !== 2) {
    // No slash in model_id, keep flat
    return;
  }
  const [vendor, model] = parts;
  const subdirPath = `${vendor}/${model}.json`;

  if (subdirFiles.has(subdirPath)) {
    // Pre-existing subdirectory file exists — delete our flat duplicate
    fs.unlinkSync(flatPath);
    deleted++;
  } else {
    // No existing subdirectory — create the subdir structure
    const targetDir = path.join(modelsDir, vendor);
    if (!fs.existsSync(targetDir)) {
      fs.mkdirSync(targetDir, { recursive: true });
    }
    const targetPath = path.join(targetDir, `${model}.json`);
    fs.renameSync(flatPath, targetPath);
    subdirFiles.add(subdirPath);
    converted++;
  }
});

console.log(`Deleted (duplicated pre-existing): ${deleted}`);
console.log(`Converted to subdirectory: ${converted}`);

// Rebuild index.json openrouter models list from all disk files
const allDirFiles = [];
fs.readdirSync(modelsDir).filter(f => {
  const p = path.join(modelsDir, f);
  return fs.statSync(p).isDirectory();
}).forEach(d => {
  const dPath = path.join(modelsDir, d);
  fs.readdirSync(dPath).forEach(f => {
    if (f.endsWith('.json')) {
      allDirFiles.push(`${d}/${f}`);
    }
  });
});
allDirFiles.sort();

const index = JSON.parse(fs.readFileSync(indexPath, 'utf-8'));
const idx = index.platforms.findIndex(p => p.code === 'openrouter');
if (idx >= 0) {
  index.platforms[idx].models = allDirFiles;
}
const now = Math.floor(Date.now() / 1000);
index.last_updated = now;
fs.writeFileSync(indexPath, JSON.stringify(index, null, 2) + "\n");

console.log(`\nIndex rebuilt with ${allDirFiles.length} files`);
console.log(`Remaining flat files: ${fs.readdirSync(modelsDir).filter(f => f.endsWith('.json')).length}`);

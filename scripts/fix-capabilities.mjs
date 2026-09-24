#!/usr/bin/env node
import fs from "fs";
import path from "path";

const modelsDir = "src-tauri/defaults/registry/platforms/openrouter/models";

const files = fs.readdirSync(modelsDir).filter(f => f.endsWith('.json'));
let fixedCapabilities = 0;
let fixedPrice = 0;
let skipped = 0;

files.forEach(f => {
  const filePath = path.join(modelsDir, f);
  const data = JSON.parse(fs.readFileSync(filePath, 'utf-8'));
  let changed = false;

  // Fix capabilities: must be array
  if (!data.capabilities || !Array.isArray(data.capabilities)) {
    data.capabilities = [];
    fixedCapabilities++;
    changed = true;
  }

  // Fix free models: must have at least input price
  if (data.price && !data.price.input) {
    if (data.price.output === 0 || data.price.output === '0' || !data.price.output) {
      // Truly free - give nominal price of 0.00000001 $/token
      data.price.input = 0.00000001;
      fixedPrice++;
      changed = true;
    }
  }

  if (changed) {
    fs.writeFileSync(filePath, JSON.stringify(data, null, 2) + "\n");
  } else {
    skipped++;
  }
});

console.log(`Fixed capabilities: ${fixedCapabilities}, Fixed price: ${fixedPrice}, Unchanged: ${skipped}, Total: ${files.length}`);

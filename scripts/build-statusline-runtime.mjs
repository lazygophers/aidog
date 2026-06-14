// Generate src/components/settings/statusline-runtime.ts from the authoritative
// engine.py (scripts/statusline-golden/engine.py). The TS module exports the
// engine source as a string constant embedded verbatim into generated scripts,
// keeping a single source of truth (engine.py is also used by the golden tests).
//
// Run: node scripts/build-statusline-runtime.mjs           → write the TS file
//      node scripts/build-statusline-runtime.mjs --check    → verify in sync (CI)
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = join(HERE, "..");
const ENGINE = join(ROOT, "scripts/statusline-golden/engine.py");
const OUT = join(ROOT, "src/components/settings/statusline-runtime.ts");

const py = readFileSync(ENGINE, "utf8");
// Escape for a JS backtick template literal: backslash, backtick, ${.
const esc = py.replace(/\\/g, "\\\\").replace(/`/g, "\\`").replace(/\$\{/g, "\\${");
const ts = `// AUTO-GENERATED from scripts/statusline-golden/engine.py — DO NOT EDIT.
// Regenerate: node scripts/build-statusline-runtime.mjs
// The statusline runtime engine, embedded verbatim into generated .py scripts.
/* eslint-disable */
export const ENGINE_PY: string = \`${esc}\`;
`;

if (process.argv.includes("--check")) {
  const cur = readFileSync(OUT, "utf8");
  if (cur !== ts) {
    console.error("statusline-runtime.ts is out of sync with engine.py — run: node scripts/build-statusline-runtime.mjs");
    process.exit(1);
  }
  console.log("statusline-runtime.ts in sync");
} else {
  writeFileSync(OUT, ts);
  console.log("wrote", OUT, `(${ts.length}B)`);
}

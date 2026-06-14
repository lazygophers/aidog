// Emit a generated statusline / subagent script for a given segment-config name.
// Usage: node emit.mjs <main|subagent> <configName>  → prints script to stdout.
import {
  generateStatusLineScript,
  generateSubagentStatusLineScript,
  DEFAULT_SEGMENTS,
  DEFAULT_SUBAGENT_SEGMENTS,
} from "../../src/components/settings/statusline-gen.ts";
import { CONFIGS } from "./configs.mjs";

const [, , kind, name] = process.argv;
const cfg = CONFIGS[name];
if (!cfg) { console.error("unknown config", name); process.exit(2); }
const segs = cfg.segments
  ?? (kind === "subagent" ? DEFAULT_SUBAGENT_SEGMENTS : DEFAULT_SEGMENTS);
const out = kind === "subagent"
  ? generateSubagentStatusLineScript(segs)
  : generateStatusLineScript(segs);
process.stdout.write(out);

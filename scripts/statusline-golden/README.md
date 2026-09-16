# Statusline golden-output regression

Guards the bash→Python conversion of the statusline / subagent-statusline script
generators (`src-tauri/crates/aidog_core/src/statusline.rs`). The generated Python
scripts must produce **byte-for-byte identical stdout** to the former jq/printf/
awk/sed bash implementation.

## Layout

- `engine.py` — the rendering engine, single source of truth. Embedded verbatim
  into every generated script by the Rust generator (`include_str!` in
  `aidog_core::statusline`). Also imported directly by the golden runner.
- `configs.mjs` — representative segment layouts (defaults + all-atoms + autocolor
  + alignment + group static/dynamic + subagent).
- `inputs/*.json` — fixture stdin payloads covering boundaries: full / minimal /
  zeros / huge (K/M truncation) / edges / RTL / group-info applicable·red·n/a /
  subagent multi-task·empty·no-id·startTime variants.
- `golden/*.golden` — captured **bash** stdout, byte-for-byte. Checked in.
- `shim/date` — deterministic `date +%s` (fixed epoch) so reset-time deltas /
  task elapsed stay stable.
- `build.mjs` — runner; it builds and drives the Rust emitter
  (`cargo build -p aidog_core --example statusline_emit --no-default-features`).
  `emit-bash.mjs` — bootstrap-only bash emitter (gitignored).
- `server.mjs` — mock `/api/group-info` (separate process; the sync script runner
  would otherwise starve an in-process server).

## Run the regression

```bash
node scripts/statusline-golden/build.mjs check    # python vs golden, exit 1 on mismatch
# or: yarn test:statusline-golden
```

Determinism: fixtures use far-future `reset_at`/no `resets_at`, non-git `current_dir`,
the `date` shim, and `cwd:/`, so output is stable across machines.

## Regenerate the golden (bootstrap only)

The golden is captured from the **original bash** generators, NOT the current
Python ones. To refresh after an intentional rendering change:

```bash
# 1. Re-extract the original bash generators from git history into .bash-orig.ts
#    (lines 1620–3133 of the pre-conversion editors.tsx, with the three exports
#    SEGMENT_DEFS / SEGMENT_DEF_MAP / groupRows added) and write emit-bash.mjs
#    (it imports ./.bash-orig.ts and prints the script for <kind> <configName>).
#    Both are gitignored.
# 2. node scripts/statusline-golden/build.mjs golden --bash
```

Never run `build.mjs golden` (without `--bash`) — it would overwrite the golden
with the current Python output, defeating the regression.

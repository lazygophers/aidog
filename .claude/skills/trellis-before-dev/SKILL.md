---
name: trellis-before-dev
description: "Discovers and injects project-specific coding guidelines from .trellis/spec/ before implementation begins. Reads spec indexes, pre-development checklists, and shared thinking guides for the target package. Use when starting a new coding task, before writing any code, switching to a different package, or needing to refresh project conventions and standards."
---

Read the relevant development guidelines before starting your task.

Execute these steps:

1. **Discover packages and their spec layers**:
   ```bash
   python3 ./.trellis/scripts/get_context.py --mode packages
   ```
   - **If `get_context.py` is missing or errors → fall back to a manual listing**: `ls .trellis/spec/` and `ls .trellis/spec/*/` to enumerate packages and layers by hand, then continue.

2. **Identify which specs apply** to your task based on:
   - Which package you're modifying (e.g., `cli/`, `docs-site/`)
   - What type of work (backend, frontend, unit-test, docs, etc.)

3. **Read the spec index** for each relevant module:
   ```bash
   cat .trellis/spec/<package>/<layer>/index.md
   ```
   Follow the **"Pre-Development Checklist"** section in the index.
   - **If `<package>/<layer>/index.md` does not exist → skip that layer and continue.** Note its absence, then move on to the shared guides (step 5); a missing layer index is not a blocker.

4. **Read the specific guideline files** listed in the Pre-Development Checklist that are relevant to your task. The index is NOT the goal — it points you to the actual guideline files (e.g., `error-handling.md`, `conventions.md`, `mock-strategies.md`). Read those files to understand the coding standards and patterns.
   - **If a listed guideline file is missing → note it and read the others.** A broken pointer in the checklist does not excuse skipping the files that do exist.

5. **Always read shared guides**:
   ```bash
   cat .trellis/spec/guides/index.md
   ```

6. Understand the coding standards and patterns you need to follow, then proceed with your development plan.

This step is **mandatory** before writing any code.

## Anti-Patterns

- **Do NOT read only the index and start coding.** The index is a pointer; the rules live in the linked guideline files. Reading `index.md` without opening the files it lists means you have not loaded the spec.
- **Do NOT skip `guides/`.** Shared thinking guides (cross-layer, cross-platform, code-reuse) apply to every task regardless of package. Always read them (step 5), even when a package-specific spec exists.
- **Do NOT silently swallow a tooling failure.** If discovery or an index read fails, fall back (manual `ls` / skip-and-note) and keep going — do not abandon the spec-loading step.

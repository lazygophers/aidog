---
name: trellis-check
description: "Comprehensive quality verification: spec compliance, lint, type-check, tests, cross-layer data flow, code reuse, and consistency checks. Use when code is written and needs quality verification, before committing changes, or to catch context drift during long sessions."
---

# Code Quality Check

Comprehensive quality verification for recently written code. Combines spec compliance, cross-layer safety, and pre-commit checks.

---

## Step 1: Identify What Changed

```bash
git diff --name-only HEAD
git status
```

## Step 2: Read Applicable Specs

```bash
python3 ./.trellis/scripts/get_context.py --mode packages
```

For each changed package/layer, read the spec index and follow its **Quality Check** section:

```bash
cat .trellis/spec/<package>/<layer>/index.md
```

Read the specific guideline files referenced — the index is a pointer, not the goal.

## Step 3: Run Project Checks

Run the project's lint, type-check, and test commands. Fix any failures before proceeding.

**If a check fails:**
- **Test is flaky** (intermittent pass/fail) → rerun up to 3 times. If it still fails on the 3rd run, mark it as a real failure and STOP — do not assume flakiness past 3 retries.
- **Linter reports a false positive** (e.g. clippy mis-flags) → you MUST add an inline suppression (`#[allow(...)]` / equivalent) WITH a comment stating the reason. Never silence a check globally or without justification.

## Step 4: Review Against Checklist

### Code Quality

- [ ] Linter passes?
- [ ] Type checker passes (if applicable)?
- [ ] Tests pass?
- [ ] No debug logging left in?
- [ ] No suppressed warnings or type-safety bypasses?

### Test Coverage

- [ ] New function → unit test added?
- [ ] Bug fix → regression test added?
- [ ] Changed behavior → existing tests updated?

### Spec Sync

- [ ] Does `.trellis/spec/` need updates? (new patterns, conventions, lessons learned)

> "If I fixed a bug or discovered something non-obvious, should I document it so future me won't hit the same issue?" → If YES, update the relevant spec doc.

## Step 5: Cross-Layer Dimensions (if applicable)

Skip this step if your change is confined to a single layer.

### A. Data Flow (changes touch 3+ layers)

- [ ] Read flow traces correctly: Storage → Service → API → UI
- [ ] Write flow traces correctly: UI → API → Service → Storage
- [ ] Types/schemas correctly passed between layers?
- [ ] Errors properly propagated to caller?

### B. Code Reuse (modifying constants, creating utilities)

- [ ] Searched for existing similar code before creating new?
  ```bash
  grep -r "pattern" src/
  ```
- [ ] If 2+ places define same value → extracted to shared constant?
- [ ] After batch modification, all occurrences updated?

### C. Import/Dependency (creating new files)

- [ ] Correct import paths (relative vs absolute)?
- [ ] No circular dependencies?

### D. Same-Layer Consistency

- [ ] Other places using the same concept are consistent?

---

> 🔴 **CHECKPOINT (before Step 6):** If ANY check is still red (lint / type / test / cross-layer), you are FORBIDDEN from declaring the check passed. A red check is a blocking failure — fix it or escalate the blocker; never report "passed with known issues".

## Step 6: Report and Fix

Report violations found and fix them directly. Re-run project checks after fixes.

**If a fix re-breaks another check** (whack-a-mole): stop patching blindly. Re-read the relevant spec (Step 2) to find the root contract, then fix once at the source instead of chasing symptoms.

### Do Not

- ❌ Do not run `git commit --no-verify` or otherwise bypass pre-commit checks.
- ❌ Do not comment out, `skip`, or delete a failing test to make the suite "green".
- ❌ Do not use `as any` / `@ts-ignore` / unexplained `unwrap()` to dodge type or safety errors.
- ❌ Do not declare the check passed while any check is red (see CHECKPOINT above).

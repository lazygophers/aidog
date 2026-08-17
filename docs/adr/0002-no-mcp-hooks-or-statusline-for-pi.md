# pi gets no MCP, hooks, statusline, or cc-switch import

aidog integrates Claude Code and Codex across thirteen touchpoints. pi reaches parity on nine of
them; four are deliberately left out because pi has no mechanism to integrate with, not because the
work was deferred.

**MCP.** pi ships none, by design: "It intentionally does not include built-in MCP, sub-agents,
permission popups, plan mode, to-dos, or background bash" (`packages/coding-agent/docs/usage.md:304`).
Bridging it would mean aidog authoring and maintaining a TypeScript extension against an API that
moves fast, in order to add back something upstream removed on purpose.

**Hooks.** pi has no configuration-driven hooks. What it calls hooks are TypeScript event handlers
registered inside an extension — `pi.on("session_start")`, `pi.on("before_provider_request")`,
`pi.on("tool_call")` (`docs/extensions.md:507,678`). There is no settings key aidog could write.

**Statusline.** No such concept; the string does not appear anywhere in pi's documentation. The
nearest equivalent is an extension calling `setStatus`/`setWidget` (`docs/extensions.md:2966`).

**cc-switch import.** Upstream cc-switch supports Claude Code, Codex, OpenCode, OpenClaw, Grok Build
and Hermes Agent — not pi. There is nothing to import.

## Consequences

These four dimensions must render as explicitly unsupported for pi in the UI rather than as empty or
broken states, so that a future reader sees a decision rather than a gap. Skills are unaffected and
need no aidog work at all: the `skills` CLI installs into `~/.agents/skills/`, which pi scans
natively (`docs/skills.md`, Locations), so every globally installed skill is visible to pi
automatically. pi has no per-skill enable/disable concept, so aidog's Skills page shows pi as a
static always-on badge rather than a toggle.

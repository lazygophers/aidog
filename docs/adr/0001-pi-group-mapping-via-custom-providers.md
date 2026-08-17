# Map aidog Groups onto pi by generating one custom provider per Group

pi resolves its API endpoint only from the single global `~/.pi/agent/models.json`
(`packages/coding-agent/src/config.ts:528-530`); it has no per-project models file, no
`ANTHROPIC_BASE_URL`-style environment variable, and no `--base-url` CLI flag
(`packages/coding-agent/src/cli/args.ts:90-94`). aidog is multi-Group, so N Groups must coexist in
one file. We generate one custom provider per Group, named `aidog-<group>`, each carrying the
proxy's base URL and the Group name as a literal `apiKey` with `authHeader: true`, so pi sends
`Authorization: Bearer <group>`. Launching is `pi --provider aidog-<group>`.

## Considered Options

**Override the built-in `anthropic` provider's `baseUrl`** (pi's documented approach for proxies,
`docs/models.md:300-314`). Zero maintenance — pi's own model catalogue and auth are preserved — but
only one Group can be active at a time, and the token would have to arrive via `ANTHROPIC_AUTH_TOKEN`,
which `~/.pi/agent/auth.json` silently overrides when the user has any stored Anthropic credential
(`docs/providers.md:139`, `packages/ai/src/providers/anthropic.ts:19-38`).

**One `PI_CODING_AGENT_DIR` per Group.** Cleanest isolation, but that variable relocates the entire
agent directory — `auth.json`, `AGENTS.md`, session history, `models-store.json` — so a user running
under an aidog Group would lose their pi login and history.

## Consequences

pi requires a non-built-in provider to carry its own `models` array, so aidog must supply the model
list: the union of the Group's effective platform models, falling back to the preset
`model_list.default` when the Group has none. Provider `headers` also carry an explicit
`User-Agent: pi (<platform> <release>; <arch>)` — pi otherwise only sets that header for the
`kimi-coding` provider (`packages/ai/src/api/anthropic-messages.ts`, `mergeClientHeaders`) and would
fall through to the anonymous Stainless SDK default, leaving the matching `pi_cli` client_type entry
with nothing distinctive to emulate.

Because the `apiKey` is a literal Group name rather than an environment variable, `auth.json` cannot
shadow it: that file is keyed by provider id, and `aidog-<group>` never collides with a built-in id.

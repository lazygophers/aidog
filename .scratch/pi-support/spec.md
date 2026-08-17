# Spec — pi Client Support

**Triage label:** `ready-for-agent`

**Primary sources:** `.scratch/pi-support/research/pi-cli-config.md` (pi v0.84.2, SHA
`914cf1472e715297caa30db4b9535d534a9eb718`), plus this session's follow-up reads of
`packages/coding-agent/docs/{usage,settings,skills,extensions}.md` and
`packages/ai/src/utils/pi-user-agent.ts` at the same tag.

**Decisions of record:** `docs/adr/0001-pi-group-mapping-via-custom-providers.md`,
`docs/adr/0002-no-mcp-hooks-or-statusline-for-pi.md`. Vocabulary: `CONTEXT.md`.

## Problem Statement

A user who runs pi cannot point it at aidog. Every other Client aidog supports can be launched
against a Group with one copied command; pi cannot, because pi resolves its endpoint only from a
single global `models.json` and offers no base-URL environment variable or CLI flag. Today the user
would have to hand-write that JSON, work out that the `/v1` suffix rule is inverted between pi's
Anthropic and OpenAI wire formats, and re-edit the file by hand every time they switch Group. None
of aidog's per-Client affordances — install/upgrade, config editing, launch commands, the sidebar —
acknowledge pi exists.

## Solution

pi becomes aidog's third Client, at parity with Claude Code and Codex on every touchpoint pi has a
mechanism for. aidog generates one pi Provider per Group in pi's `models.json`, so all Groups coexist
and the user switches by launching `pi --provider aidog-<group>` — a command aidog offers for copy
next to the existing Claude and Codex ones. The Group marked as Default Group additionally becomes
pi's `defaultProvider`, so bare `pi` routes through aidog.

Four touchpoints are explicitly unsupported because pi has no mechanism to integrate with — MCP,
hooks, statusline, cc-switch import (see ADR 0002). Skills need no work: pi natively scans
`~/.agents/skills/`, where the `skills` CLI already installs.

## User Stories

1. As a pi user, I want aidog to detect whether pi is installed and show its version, so that I can
   see at a glance whether my environment is ready.
2. As a pi user, I want to install pi from inside aidog, so that I do not have to look up the npm
   package name.
3. As a pi user, I want to upgrade pi from inside aidog, so that I stay current without leaving the
   app.
4. As a pi user, I want aidog to warn me when it finds conflicting pi installations, so that I know
   which binary will actually run.
5. As a user with several Groups, I want each Group to appear in pi as its own selectable provider,
   so that I can switch Groups without editing any file.
6. As a user, I want the pi provider for a Group to carry that Group's name as its credential, so
   that aidog routes my request to the right Group.
7. As a user who has logged into pi with a Claude subscription, I want aidog's Groups to work
   regardless, so that my stored pi credentials do not silently hijack aidog's routing.
8. As a user, I want to copy a ready-to-run pi launch command for any Group, so that I can paste it
   into a terminal and start working.
9. As a user, I want my Group's configured environment variables to be included in that launch
   command, so that pi runs with the environment I expect.
10. As a user, I want the Group I marked as Default Group to become pi's default provider, so that
    typing bare `pi` routes through aidog.
11. As a user, I want unmarking the Default Group to remove only aidog's own default-provider entry,
    so that a default provider I set myself by hand survives.
12. As a user routing to an Anthropic-protocol Platform, I want pi to receive a base URL without a
    version suffix, so that pi's SDK does not produce a doubled path.
13. As a user routing to an OpenAI-protocol Platform, I want pi to receive a base URL that includes
    the version prefix, so that requests reach the right path.
14. As a user, I want to choose which wire protocol a Group's pi provider speaks, so that I can
    match it to the Platforms in that Group.
15. As a user, I want the protocol choice to be a named option rather than a URL I type, so that I
    cannot get the version-suffix rule wrong.
16. As a user, I want the pi provider to list the models my Group can actually route to, so that I
    do not pick a model that returns an error.
17. As a user whose Group has no models configured yet, I want the pi provider to still offer the
    protocol's default candidates, so that I get a usable provider rather than an empty one.
18. As a user, I want pi's requests to arrive in a form my upstream accepts, so that eager tool
    input streaming and long cache retention do not break platforms that reject them.
19. As a user, I want aidog to tolerate pi-specific request fields even if pi's own settings change,
    so that a pi upgrade does not break my routing.
20. As a user, I want my upstream logs to show that a request came from pi, so that I can tell my
    Clients apart.
21. As a user, I want to edit pi's global settings from inside aidog, so that I do not hand-edit
    JSON.
22. As a user, I want a dedicated pi entry in aidog's settings navigation, so that I can find those
    settings where I find the Claude and Codex ones.
23. As a user, I want my outbound HTTP proxy applied to pi, so that pi reaches upstreams the same way
    my other Clients do.
24. As a user, I want unknown keys in my existing pi config preserved when aidog writes it, so that
    aidog does not destroy settings it does not understand.
25. As a user, I want aidog to leave pi's built-in providers untouched, so that I can still use pi
    against providers aidog does not manage.
26. As a user, I want a pi icon in the sidebar and Group list matching the Claude and Codex ones, so
    that the three Clients read as equals.
27. As a user, I want deleting a Group to remove its pi provider, so that stale entries do not
    accumulate in my `models.json`.
28. As a user, I want my globally installed skills to be available in pi, so that I do not maintain
    a separate skill set per Client.
29. As a user, I want aidog's Skills page to show pi's always-on status honestly rather than a
    toggle that does nothing, so that I am not misled.
30. As a user, I want MCP, hooks, and statusline to be shown as unsupported for pi rather than empty,
    so that I understand it is a pi limitation and not an aidog bug.
31. As a non-Chinese-speaking user, I want every new pi string translated into all eight supported
    languages, so that the interface is not half-translated.
32. As a user reading the docs, I want a pi setup page in every documentation language, so that I can
    follow it in my own language.

## Implementation Decisions

**Group mapping.** One pi Provider per Group, named `aidog-<group_key>`, all written into the single
`~/.pi/agent/models.json`. `apiKey` is the literal Group name and `authHeader` is `true`, so pi emits
`Authorization: Bearer <group>` with no environment variable involved. This also sidesteps pi's
`auth.json`-over-environment precedence, because `auth.json` is keyed by provider id and
`aidog-<group>` never collides with a built-in id. Full rationale and rejected alternatives in
ADR 0001.

**Protocol selection.** Per-Group, stored in the Group's existing `extra` JSON blob — no schema
migration, matching how Platform-level `peak_hours` is already stored. The four values map to pi's
`api` field: `anthropic-messages`, `openai-completions`, `openai-responses`,
`google-generative-ai`. aidog derives the base URL from the choice: the Anthropic form gets the proxy
root with no version suffix, the OpenAI forms get the root plus the version prefix. The user never
types a URL. Note that pi's own documentation gives an incorrect example here; the built-in provider
constants in pi's source are the authority.

**Model list.** The provider's required `models` array is the union of the Group's effective Platform
models, falling back to the protocol's preset `model_list.default` when the Group has none, so an
empty Group still yields a usable provider.

**Request shaping — both sides.** The generated provider carries `compat` flags disabling eager tool
input streaming and long cache retention, so pi does not emit them. aidog's adapter layer
additionally tolerates them if they arrive anyway.

**Client identity.** The generated provider sets an explicit `User-Agent` header of the form
`pi (<platform> <release>; <arch>)`, matching pi's own `getPiUserAgent()` format. pi only sets that
header itself for one built-in provider and would otherwise fall through to an anonymous SDK default.
A matching `pi_cli` entry joins aidog's client-type table.

**Default Group.** Written as `defaultProvider` in pi's global `settings.json`. Clearing it removes
the key only when its value is an `aidog-` provider, preserving a user-set value — the same guard the
Codex integration already uses.

**Settings file handling.** aidog merges into pi's existing global `settings.json` rather than
replacing it: `defaultProvider`, `httpProxy` (pi has a native setting for this, unlike Codex which
needs process environment), and nothing else. Unknown keys are preserved. Likewise `models.json`
carries only `aidog-*` providers; pi's built-ins and any user-authored providers are untouched.

**Group deletion.** Removing a Group removes its `aidog-<group>` provider, mirroring the existing
Codex profile cleanup, which sweeps by name prefix and never touches the user's own baseline file.

**CLI management.** pi joins aidog's tool registry with npm package `@earendil-works/pi-coding-agent`
and binary name `pi`, feeding the existing install / upgrade / version-check / conflict-diagnosis
paths.

**Skills.** No aidog work. The `skills` CLI installs into `~/.agents/skills/`, which pi scans
natively. pi has no per-skill enable concept, so the Skills page shows pi as a static always-on badge
rather than a toggle.

**Explicitly not built.** MCP, hooks, statusline, cc-switch import — pi has no mechanism for any of
them. Each must render as explicitly unsupported rather than empty. See ADR 0002.

## Testing Decisions

A good test here asserts on the artefact a user would inspect — the JSON aidog writes, the command
string aidog offers — not on how the code arrived there. Tests must not reach into intermediate
helpers or assert on call order.

**Seam 1 (Rust, primary).** A single pure function takes the Groups, the Default Group, the proxy
port and proxy settings, and returns both file contents. Every decision above is observable in its
return value, so the whole feature is testable without touching the filesystem. Cases to cover: one
provider per Group with correct naming; the Group name landing in `apiKey` with `authHeader` set; the
version-suffix rule differing between the Anthropic and OpenAI protocol choices; the model list
falling back to preset defaults for an empty Group; `compat` flags present; the `User-Agent` header
present and correctly shaped; `defaultProvider` set for the Default Group and absent otherwise;
merging into a settings file that already has unrelated keys leaving those keys intact; a user-set
`defaultProvider` surviving while an `aidog-` one is cleared; a deleted Group's provider gone while
built-in and user-authored providers remain. Prior art: the Codex profile builder and its test module
are the same shape — a pure builder plus a thin IO wrapper.

**Seam 2 (TypeScript).** The pi launch-command builder, tested as a pure string function: Group name
quoting, protocol reflected correctly, Group environment variables prefixed. Prior art: the existing
Codex command builder alongside it.

**Not new seams.** The tool registry entry, the client-type entry, the icon and the translation keys
are data. They are covered by the existing registry assertion and the existing i18n checker; adding
bespoke tests for them would test the data, not behaviour.

## Out of Scope

- MCP, hooks, statusline and cc-switch import for pi (ADR 0002). Each renders as unsupported.
- Any aidog-authored pi extension. If the three unsupported touchpoints are ever revisited, that is a
  separate effort with its own research.
- Project-level Group binding via pi's per-project settings file. pi supports it and neither Claude
  Code nor Codex has an equivalent in aidog; introducing it here would add a Client-specific concept
  mid-integration. Worth a separate ticket later.
- Writing per-skill entries into pi's settings. Unnecessary — the shared skills directory is already
  on pi's native scan path.
- Model-level protocol overrides. pi supports per-model `api`, but per-Group selection covers the
  need at a fraction of the UI cost.

## Further Notes

Two items from the original research remain unresolved and were not advanced this session: pi's
concrete LLM-request timeout and retry values were not located in primary sources, and `models.json`
has schema validation in pi's code but no exportable schema file aidog could validate against. Neither
blocks this work.

pi releases fast — 50 tags by v0.84.2. The integration deliberately depends only on pi's stable
configuration surface (`models.json` provider fields, `settings.json` keys, the skills scan paths) and
not on its extension API, which is the fastest-moving part.

# aidog

aidog is a desktop control plane for AI coding CLIs. It runs a local proxy that routes a CLI's LLM
requests to whichever upstream provider the user has configured, and it writes the configuration
files those CLIs read so the user never edits them by hand.

## Language

### Routing

**Group**:
A named routing target. A CLI authenticates to the local proxy with the group name as its bearer
token, and the proxy picks an upstream from the group's platforms.
_Avoid_: profile, workspace, account

**Platform**:
One configured upstream LLM provider — a base URL, a key, a protocol, and a model set.
_Avoid_: provider, vendor, endpoint

**Protocol**:
The wire format a platform speaks (`anthropic`, `openai`, `glm_coding`, …). Determines both request
translation and which URL suffix the proxy appends.
_Avoid_: API type, format, wire API

**Client**:
An AI coding CLI that aidog configures and proxies for — currently Claude Code, Codex, and pi.
_Avoid_: agent, tool, harness

**Default Group**:
The one Group whose configuration aidog merges into a Client's own global config file, so the user
can launch that Client bare and still be routed.
_Avoid_: primary group, active group

### pi integration

**pi Provider**:
An entry aidog generates in pi's `models.json`, named `aidog-<group>`, that points pi at the local
proxy for one Group. pi's own built-in providers are untouched.
_Avoid_: pi profile, pi config entry

**Touchpoint**:
One dimension along which aidog integrates a Client — installation, config generation, launch
command, skills, icon, and so on. Used to check a new Client for parity against existing ones.
_Avoid_: integration point, surface, feature

### Middleware

**Middleware Rule**:
One proxy rewrite rule: a Condition Tree, an Action Chain, and an Applies To filter. It affects
only the request sent upstream and the body returned downstream — never log recording.
_Avoid_: rule type, filter rule (the old 8-type names)

**Condition Tree**:
A nested ALL/ANY boolean combination whose leaves each match one target (request_body /
request_headers / response_body / response_headers / status / model) with contains/regex/exact.
All leaves in one rule must belong to the same phase (request-side or response-side).
_Avoid_: condition list, matcher

**Action Chain**:
An ordered sequence of actions (mask / block / warn / inject / override / classify) executed when
the tree matches. block and classify are terminal: they stop the chain and all later rules.
_Avoid_: action type

**Applies To**:
The filter on a rule — platforms, groups, models — that decides where it runs. Empty means all;
multiple values mean any-of; rules stack by priority. Replaces the old three-level cascade.
_Avoid_: scope, scope_ref

**Builtin Rule**:
A rule shipped by aidog. It can only be enabled or disabled — never edited or deleted. Upgrades
overwrite its content by name while preserving the user's disabled state.
_Avoid_: default rule, seed rule

**Failed Rule**:
A leftover rule from the old 8-type model that could not be translated. It is shown as failed in
the list for the user to delete manually.
_Avoid_: broken rule, invalid rule

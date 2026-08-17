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

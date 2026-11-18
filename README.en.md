<div align="center">

# AiDog

**Manage AI platforms on your machine and forward client requests to the platforms you choose.**

A desktop app providing a local proxy, group routing, model mapping, request logging, and usage estimation.

[Documentation](https://lazygophers.github.io/aidog/en/) · [Latest release](https://github.com/lazygophers/aidog/releases/latest) · [Issues](https://github.com/lazygophers/aidog/issues)

[简体中文](README.md) · English · [Français](README.fr.md) · [Deutsch](README.de.md) · [Русский](README.ru.md) · [العربية](README.ar.md) · [Español](README.es.md) · [日本語](README.ja.md)

</div>

AiDog runs a proxy service on your computer. You store upstream platforms and API keys in AiDog, then point Claude Code, Codex, pi, or other compatible clients at the local proxy. Requests are still sent to the selected upstream; running locally does not mean requests stay on your machine.

## Who it is for

- You use multiple AI services and want to manage platform credentials and models in one app.
- You want different clients to use different groups, model mappings, or candidate platforms.
- You need to inspect request status, usage, estimated cost, and routing results.
- You want Claude Code, Codex, or pi to share one local proxy entry point.

## What it does

![AiDog home overview](screenshots/home-light.png)

![AI platform management](screenshots/platforms-light.png)

*Illustrative UI with mock data.*

### Local proxy and routing

- Manage built-in platform presets, and configure custom platforms.
- Isolate client requests with groups and associate each group with its own platforms.
- Select candidate platforms by model mapping, endpoint protocol, and availability.
- Multi-model slots support ordered model chains; subsequent candidates are tried when a request fails.
- Supports forwarding and adaptation for OpenAI, Anthropic, Gemini, and other protocols; actual capability depends on the platform endpoint and request type.
- Supports middleware rules, time-based model configuration, and routing health scheduling.

### Logging and statistics

- View proxy requests plus separate records for tests, balance queries, and model-list requests.
- Keep or clear the raw content on both sides of a request according to settings; sensitive request headers are redacted.
- Estimate cost from response usage and built-in model prices. Estimates are not a substitute for upstream billing.
- View platform balances, quotas, and per-group usage statistics; these metrics use different counting methods and are not directly equivalent.

### Clients and tools

- Manage Claude Code, Codex, and pi configurations, and generate connection instructions per group.
- Manage MCP servers, skills, notifications, and system settings.
- Optionally run a headless kernel, operated through a management API restricted to local access.

## Install

Download the installer matching your system from [GitHub Releases](https://github.com/lazygophers/aidog/releases/latest). The packages, architectures, and requirements available per system are listed on that release page.

After starting AiDog, the local proxy listens on `127.0.0.1:9890` by default. Only the loopback address is listened on by default; do not forward the management API to the public internet.

## Quick start

1. Add a platform under "AI Platforms" and fill in the protocol, Base URL, and upstream API key.
2. Create a group on the platform page and add platforms to it.
3. In your client, use the connection command generated for that group, or follow the client integration guide to configure the local proxy address and group credentials.
4. Send one request and confirm the group, upstream platform, model, and result in the proxy log.

[Installation guide](https://lazygophers.github.io/aidog/en/getting-started/installation) · [First configuration](https://lazygophers.github.io/aidog/en/getting-started/first-config) · [Verify your first request](https://lazygophers.github.io/aidog/en/getting-started/first-request)

## Data and privacy

AiDog stores settings and databases on your machine. Proxy requests forward necessary content to the selected upstream service. Whether request logs keep request and response bodies depends on log settings; with raw content logging off, metadata such as status, model, and usage can still be recorded. See [Data and privacy](https://lazygophers.github.io/aidog/en/core-concepts/data-privacy).

## Build from source

Install the tools and system dependencies first per the [official Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
git clone https://github.com/lazygophers/aidog.git
cd aidog
yarn install
yarn tauri dev
```

Build an installer:

```bash
yarn tauri build
```

## License

AiDog is licensed under [GNU AGPL-3.0-or-later](LICENSE).

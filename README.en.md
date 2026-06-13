<div align="center">

# 🐕 AiDog

**Unified AI API Gateway**

Multi-platform aggregation · Smart routing · Usage analytics — a cross-platform desktop app to manage keys, requests, and spend across all your AI platforms

[![Documentation](https://img.shields.io/badge/docs-lazygophers.github.io/aidog-0a66c2?logo=sphinx&logoColor=white)](https://lazygophers.github.io/aidog/en/)
[![GitHub Release](https://img.shields.io/github/v/release/lazygophers/aidog?logo=github)](https://github.com/lazygophers/aidog/releases)
[![License](https://img.shields.io/badge/license-MIT-blue)](#license)

[简体中文](README.md) · [English](README.en.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Русский](README.ru.md) · [العربية](README.ar.md) · [Español](README.es.md) · [日本語](README.ja.md)

</div>

> 📖 **Full documentation**: <https://lazygophers.github.io/aidog/en/>

AiDog is a Tauri-based desktop AI API gateway that unifies the management, routing, and monitoring of requests across 50+ AI platforms. It consolidates scattered API keys, model mappings, load balancing, and usage analytics into a single app — no backend service, no cloud, all data stored locally.

## ✨ Features

- **Multi-platform aggregation** — 50+ platform presets (Anthropic / OpenAI / DeepSeek / GLM / Kimi / MiniMax / Qwen, etc.), one-click setup
- **Smart grouping** — match requests by Bearer token / path; Failover and Load Balance routing
- **Model mapping** — transparent model name substitution (e.g. `claude-sonnet-4` → `deepseek-chat`)
- **Protocol conversion** — bidirectional conversion between OpenAI Chat / Completions / Responses, Anthropic, and Gemini protocols
- **Load balancing & failover** — automatic retry across platforms on failure, circuit breaking / tri-state management / exponential backoff
- **Usage monitoring** — token stats, cost estimation, per-platform balance queries, Coding Plan quota display
- **Request logging** — three-level granularity (user original request / upstream request / summary), each with independent toggle and retention
- **Middleware rule engine** — inbound/outbound rules: normalization, override, redaction, injection, sensitive-word filtering, error detection
- **Coding assistant integration** — native one-click support for Claude Code, OpenAI Codex, and other coding assistants
- **i18n & themes** — 8 languages (incl. Arabic RTL), Liquid Glass and other themes with light/dark modes

## 🚀 Quick Start

### Download & install

Grab the installer for your platform (macOS / Windows / Linux) from [GitHub Releases](https://github.com/lazygophers/aidog/releases).

See the [Installation Guide](https://lazygophers.github.io/aidog/en/getting-started/installation).

### Three steps to go

1. **Add a platform** — enter an AI platform's API key and endpoint
2. **Configure the proxy** — point your client's API base URL at the local proxy:
   ```
   http://127.0.0.1:9876/proxy/v1
   ```
3. **Start using** — requests are routed, metered, and logged automatically

Verify with curl:

```bash
curl http://127.0.0.1:9876/proxy/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer any-value" \
  -d '{"model": "gpt-4o", "messages": [{"role": "user", "content": "Hello!"}]}'
```

> 💡 The API key in the proxy URL can be any value — AiDog forwards with your configured real key.

📖 Full tutorial: [Quick Start](https://lazygophers.github.io/aidog/en/getting-started/quick-start).

## 🧩 Tech Stack

| Layer | Technology |
| --- | --- |
| Desktop framework | Tauri 2.0 |
| Frontend | React 19 + TypeScript + Vite |
| Backend | Rust + Axum proxy + SQLite storage |
| Build | Yarn + Vite |

## 🛠️ Development

```bash
yarn                          # install frontend deps
yarn tauri dev                # launch desktop app (dev)
yarn build                    # frontend build (tsc && vite build)
cd src-tauri && cargo build   # build Rust backend
cd src-tauri && cargo clippy  # Rust lint (warnings must be clean)
cd src-tauri && cargo test    # Rust tests
```

Prerequisites: Node.js ≥ 18, Yarn 4.x, Rust toolchain, Tauri CLI.

## 📚 Documentation

Full docs site: <https://lazygophers.github.io/aidog>

- [Quick Start](https://lazygophers.github.io/aidog/en/getting-started/quick-start)
- [Platform Protocols](https://lazygophers.github.io/aidog/en/platforms/protocols)
- [Groups & Routing](https://lazygophers.github.io/aidog/en/groups/routing-rules)
- [Codex Integration](https://lazygophers.github.io/aidog/en/proxy/codex-integration)
- [Usage Stats & Pricing](https://lazygophers.github.io/aidog/en/stats/usage-stats)

## 🌍 Languages

| Language | README |
| --- | --- |
| 简体中文 | [README.md](README.md) |
| English | [README.en.md](README.en.md) |
| Français | [README.fr.md](README.fr.md) |
| Deutsch | [README.de.md](README.de.md) |
| Русский | [README.ru.md](README.ru.md) |
| العربية | [README.ar.md](README.ar.md) |
| Español | [README.es.md](README.es.md) |
| 日本語 | [README.ja.md](README.ja.md) |

## Recommended IDE

[VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer).

## License

MIT

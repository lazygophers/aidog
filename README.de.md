<div align="center">

# 🐕 AiDog

**Unified AI API Gateway**

Multi-Plattform-Aggregation · Smartes Routing · Usage-Analytics — eine plattformübergreifende Desktop-App zur Verwaltung von Keys, Requests und Ausgaben über alle deine AI-Plattformen hinweg

[![Documentation](https://img.shields.io/badge/docs-lazygophers.github.io/aidog-0a66c2?logo=sphinx&logoColor=white)](https://lazygophers.github.io/aidog/de/)
[![GitHub Release](https://img.shields.io/github/v/release/lazygophers/aidog?logo=github)](https://github.com/lazygophers/aidog/releases)
[![License](https://img.shields.io/badge/license-MIT-blue)](#license)

[简体中文](README.md) · [English](README.en.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Русский](README.ru.md) · [العربية](README.ar.md) · [Español](README.es.md) · [日本語](README.ja.md)

</div>

> 📖 **Vollständige Dokumentation**: <https://lazygophers.github.io/aidog/de/>

AiDog ist ein Tauri-basiertes Desktop-AI-API-Gateway, das Verwaltung, Routing und Überwachung von Requests über 50+ AI-Plattformen vereinheitlicht. Es konsolidiert verstreute API-Keys, Modell-Mappings, Load Balancing und Usage-Analytics in einer App — kein Backend-Service, keine Cloud, alle Daten lokal gespeichert.

## ✨ Features

- **Multi-Plattform-Aggregation** — 50+ Plattform-Presets (Anthropic / OpenAI / DeepSeek / GLM / Kimi / MiniMax / Qwen usw.), Ein-Klick-Setup
- **Smartes Grouping** — Requests per Bearer-Token / Pfad matchen; Failover- und Load-Balance-Routing
- **Modell-Mapping** — transparente Modellnamens-Substitution (z.B. `claude-sonnet-4` → `deepseek-chat`)
- **Protokoll-Konvertierung** — bidirektionale Konvertierung zwischen OpenAI Chat / Completions / Responses, Anthropic und Gemini
- **Load Balancing & Failover** — automatischer Retry plattformübergreifend bei Fehler, Circuit Breaking / Drei-Zustands-Verwaltung / exponentielles Backoff
- **Usage-Monitoring** — Token-Statistiken, Kosten-Schätzung, plattformspezifische Saldo-Abfragen, Coding-Plan-Quota-Anzeige
- **Request-Logging** — dreistufige Granularität (ursprünglicher User-Request / Upstream-Request / Zusammenfassung), jeweils unabhängiger Toggle und Aufbewahrung
- **Middleware-Regel-Engine** — Inbound-/Outbound-Regeln: Normalisierung, Override, Redaction, Injection, Sensitive-Word-Filterung, Fehlererkennung
- **Coding-Assistant-Integration** — native Ein-Klick-Unterstützung für Claude Code, OpenAI Codex und weitere
- **i18n & Themes** — 8 Sprachen (inkl. Arabisch RTL), Liquid Glass und weitere Themes mit Hell/Dunkel-Modi

## 🚀 Quick Start

### Download & Installation

Lade den Installer für deine Plattform (macOS / Windows / Linux) von [GitHub Releases](https://github.com/lazygophers/aidog/releases) herunter.

Siehe [Installationsanleitung](https://lazygophers.github.io/aidog/de/getting-started/installation).

### Drei Schritte

1. **Plattform hinzufügen** — API-Key und Endpoint einer AI-Plattform eingeben
2. **Proxy konfigurieren** — Base URL des Clients auf den lokalen Proxy setzen:
   ```
   http://127.0.0.1:9876/proxy/v1
   ```
3. **Verwenden** — Requests werden automatisch geroutet, gemessen und geloggt

Mit curl verifizieren:

```bash
curl http://127.0.0.1:9876/proxy/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer any-value" \
  -d '{"model": "gpt-4o", "messages": [{"role": "user", "content": "Hello!"}]}'
```

> 💡 Der API-Key in der Proxy-URL kann ein beliebiger Wert sein — AiDog leitet mit deinem konfigurierten echten Key weiter.

📖 Vollständiges Tutorial: [Quick Start](https://lazygophers.github.io/aidog/de/getting-started/quick-start).

## 🧩 Tech Stack

| Schicht | Technologie |
| --- | --- |
| Desktop-Framework | Tauri 2.0 |
| Frontend | React 19 + TypeScript + Vite |
| Backend | Rust + Axum Proxy + SQLite Storage |
| Build | Yarn + Vite |

## 🛠️ Entwicklung

```bash
yarn                          # Frontend-Dependencies installieren
yarn tauri dev                # Desktop-App starten (Dev)
yarn build                    # Frontend-Build (tsc && vite build)
cd src-tauri && cargo build   # Rust-Backend bauen
cd src-tauri && cargo clippy  # Rust-Lint (Warnungen müssen clean sein)
cd src-tauri && cargo test    # Rust-Tests
```

Voraussetzungen: Node.js ≥ 18, Yarn 4.x, Rust-Toolchain, Tauri CLI.

## 📚 Dokumentation

Vollständige Docs-Site: <https://lazygophers.github.io/aidog>

- [Quick Start](https://lazygophers.github.io/aidog/de/getting-started/quick-start)
- [Plattform-Protokolle](https://lazygophers.github.io/aidog/de/platforms/protocols)
- [Gruppen & Routing](https://lazygophers.github.io/aidog/de/groups/routing-rules)
- [Codex-Integration](https://lazygophers.github.io/aidog/de/proxy/codex-integration)
- [Usage-Stats & Pricing](https://lazygophers.github.io/aidog/de/stats/usage-stats)

## 🌍 Sprachen

| Sprache | README |
| --- | --- |
| 简体中文 | [README.md](README.md) |
| English | [README.en.md](README.en.md) |
| Français | [README.fr.md](README.fr.md) |
| Deutsch | [README.de.md](README.de.md) |
| Русский | [README.ru.md](README.ru.md) |
| العربية | [README.ar.md](README.ar.md) |
| Español | [README.es.md](README.es.md) |
| 日本語 | [README.ja.md](README.ja.md) |

## Empfohlene IDE

[VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer).

## License

MIT

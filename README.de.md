<div align="center">

# 🐕 AiDog

**Einheitliches KI-API-Gateway**

Desktop-App · Keine Cloud · 50+ Plattformen an einem Ort · Smartes Routing · Nutzungsanalyse

[![Documentation](https://img.shields.io/badge/docs-lazygophers.github.io/aidog-0a66c2?logo=sphinx&logoColor=white)](https://lazygophers.github.io/aidog/de/)
[![GitHub Release](https://img.shields.io/github/v/release/lazygophers/aidog?logo=github&label=release)](https://github.com/lazygophers/aidog/releases/latest)
[![License](https://img.shields.io/badge/license-AGPL_3.0-blue)](#license)
[![Platforms](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey?logo=tauri&logoColor=white)](https://github.com/lazygophers/aidog/releases/latest)
[![GitHub Stars](https://img.shields.io/github/stars/lazygophers/aidog?style=social)](https://github.com/lazygophers/aidog/stargazers)
[![Downloads](https://img.shields.io/github/downloads/lazygophers/aidog/total?logo=github)](https://github.com/lazygophers/aidog/releases)
[![Last Commit](https://img.shields.io/github/last-commit/lazygophers/aidog?logo=git&logoColor=white)](https://github.com/lazygophers/aidog/commits)
[![Issues](https://img.shields.io/github/issues/lazygophers/aidog?logo=github)](https://github.com/lazygophers/aidog/issues)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen?logo=github)](https://github.com/lazygophers/aidog/pulls)

[简体中文](README.md) · [English](README.en.md) · [Français](README.fr.md) · `Deutsch` · [Русский](README.ru.md) · [العربية](README.ar.md) · [Español](README.es.md) · [日本語](README.ja.md)

</div>

---

> 📖 **Vollständige Dokumentation**: <https://lazygophers.github.io/aidog/de/>

AiDog ist ein **Desktop-KI-API-Gateway** auf Basis von Tauri. Es vereint Verwaltung, Routing und Überwachung von Anfragen über 50+ KI-Plattformen — und bündelt verstreute API-Schlüssel, Modell-Mappings, Lastausgleich, Nutzungsanalyse und Coding-Assistent-Konfiguration in einer einzigen App. Kein Backend-Dienst, keine Cloud; alle Daten bleiben in einer lokalen SQLite-Datenbank.

![AiDog — Übersicht](screenshots/dashboard.png)

## Welches Problem es löst

| Ihr Schmerz | Wie AiDog ihn löst |
| --- | --- |
| API-Schlüssel über ein Dutzend Plattformen verstreut, mühsam zu wechseln | **Multi-Plattform-Aggregation** — 50+ Plattform-Presets, jeden Schlüssel an einem Ort verwalten |
| Eine Plattform fällt aus und Ihr ganzer Flow stoppt | **Failover + Lastausgleich** — Auto-Retry, Circuit Breaking, Scheduling über Plattformen |
| Claude Code / Codex / jeder Client separat konfiguriert | **Native Coding-Assistent-Integration** — Ein-Klick-Konfigurationsexport, der gesamte Traffic über den Proxy |
| Keine Ahnung, wie viel Sie monatlich ausgeben oder welche Plattform bald erschöpft ist | **Nutzungsüberwachung** — Token + Kostenschätzung + Guthaben + Coding-Plan-Quote |
| Daten sollen nicht in die Cloud oder zu Dritten | **Rein lokal** — Proxy + Datenbank auf Ihrer Maschine, null Exfiltration |

## Kernfunktionen

### 🌐 Gateway & Routing
- **Multi-Plattform-Aggregation** — 50+ Plattform-Presets (Anthropic / OpenAI / DeepSeek / GLM / Kimi / MiniMax / Qwen / SiliconFlow / OpenRouter usw.), Ein-Klick-Setup
- **Intelligente Gruppierung** — Anfragen per Bearer-Token / Pfad zuordnen; Failover und Load Balance
- **Modell-Mapping** — transparente Modellnamens-Substitution (z. B. `claude-sonnet-4` → `deepseek-chat`)
- **Protokoll-Konvertierung** — bidirektional zwischen OpenAI Chat / Completions / Responses, Anthropic und Gemini
- **Circuit Breaking & Scheduling** — automatische Abschaltung auffälliger Plattformen, Tri-State-Verwaltung, exponentielles Backoff, intelligentes Scheduling innerhalb der Gruppe
- **Middleware-Regel-Engine** — Inbound/Outbound-Regeln: Normalisierung, Override, Redaktion, Injection, Filter sensibler Wörter, Fehlererkennung, mit eingebauten Presets

### 📊 Überwachung & Statistiken
- **Nutzungsüberwachung** — Token-Statistiken, Kostenschätzung (auto Preissync + manuelles Budget)
- **Guthaben-Abfragen** — Echtzeit-Guthaben jeder Plattform abrufen
- **Coding-Plan-Quote** — DeepSeek / Kimi / GLM Coding-Plan-Quote anzeigen und Countdown
- **Anfrage-Logs** — drei Granularitätsstufen (ursprüngliche Nutzeranfrage / Upstream-Anfrage / Zusammenfassung), jeweils mit eigenem Schalter und Aufbewahrung

### 🤖 Coding-Assistent-Integration
- **Claude Code** — native Integration: Konfigurationsbearbeitung, Ein-Klick-Import/Export, StatusLine-Skripte, Hooks, per-Gruppen-Konfig-Sync
- **OpenAI Codex** — native Integration: `~/.codex/config.toml`-Editor, automatisches Responses-API-Routing
- **MCP-Verwaltung** — zentrale DB-Ablage + Per-Agent-Aktivierung + Scan & Import + Maskierung sensibler Felder
- **Skills-Verwaltung** — npx-basierte einheitliche plattformübergreifende Skills-Liste + Per-Item-Aktivierung
- **Systembenachrichtigungen** — TTS-Ansagen / Popup / Posteingang + Claude Code/Codex-Hook Ein-Klick-Injection

### 🎨 Personalisierung
- **Theme-System** — 3 Achsen: 9 Styles (Liquid Glass / Flat / Soft / Sharp / Aurora / Paper / Terminal / Bento / Sketchy) × 12 benannte Paletten (Apple Blue / Nord / Dracula / Catppuccin / Gruvbox / Tokyo Night / One Dark / Material / GitHub / Night Owl usw.) × Hell/Dunkel-Modi
- **Internationalisierung** — 8 Sprachen (inkl. Arabisch RTL)
- **Import & Export** — AES-256-GCM-verschlüsselter Einzeldatei-Container `.aidogx`, 7 Bereiche mit Per-Item-Konfliktlösung
- **Tray + Statusleiste** — Schnellaktionen aus dem System-Tray + anpassbare Statusleisten-Skripte (Python + uv)

## Installation

### Systemanforderungen

| OS | Mindestversion | Hinweise |
| --- | --- | --- |
| macOS | 12.0 (Monterey) | Intel + Apple Silicon |
| Windows | Windows 10 | x64 |
| Linux | x86_64 / aarch64 | Benötigt WebKit2GTK |

**Download** 👉 <https://github.com/lazygophers/aidog/releases/latest>

### macOS

1. Laden Sie die `.dmg` von [Releases Latest](https://github.com/lazygophers/aidog/releases/latest) herunter
2. Doppelklicken zum Öffnen, ziehen Sie **AiDog** in den Ordner `Programme`
3. Beim ersten Start **Rechtsklick** auf die App → „Öffnen" wählen (Gatekeeper umgehen — die App ist unsigniert)

> ⚠️ Falls beim ersten Start „Entwickler kann nicht verifiziert werden" erscheint: `Systemeinstellungen → Datenschutz & Sicherheit → Trotzdem öffnen`.

### Windows

1. Laden Sie den `.msi`-Installer von [Releases Latest](https://github.com/lazygophers/aidog/releases/latest) herunter
2. Doppelklick auf den Installer und den Eingaben folgen
3. Falls SmartScreen blockiert, klicken Sie auf „Weitere Informationen → Trotzdem ausführen"

### Linux

```bash
# DEB-Paket
sudo dpkg -i aidog_0.1.0_amd64.deb

# Oder AppImage
chmod +x aidog_0.1.0_amd64.AppImage
./aidog_0.1.0_amd64.AppImage
```

> Linux benötigt zunächst die Abhängigkeit WebKit2GTK: `sudo apt install libwebkit2gtk-4.1-dev` (Debian/Ubuntu).

### Erster Start

Nach der Installation starten Sie AiDog — es wird automatisch:

1. Den lokalen Proxy-Server starten (Standard `http://127.0.0.1:9876`)
2. Die lokale SQLite-Datenbank erstellen (`~/.aidog/aidog.db`)
3. Die Hauptoberfläche zeigen und Sie beim Hinzufügen der ersten Plattform anleiten

## Schnellstart (3 Schritte)

### Schritt 1: Plattform hinzufügen

![AiDog — Plattform hinzufügen](screenshots/add-platform.png)

1. Klicken Sie im linken Navigationsbereich auf **„Plattformen"**
2. Klicken Sie auf **„+ Plattform hinzufügen"**
3. Ausfüllen: **Name** (z. B. `Mein OpenAI`), **Base URL** (z. B. `https://api.openai.com/v1`, inkl. `/v1`-Versionspräfix), **API-Schlüssel**
4. Speichern

> 💡 Die Base URL enthält bereits das Versionspräfix; AiDog hängt `/chat/completions` automatisch an — der Pfad muss nicht von Hand zusammengesetzt werden.

### Schritt 2: Client auf den Proxy zeigen

In der App, die KI-APIs konsumiert, ändern Sie die API-Adresse auf die AiDog-Proxy-Adresse:

```
http://127.0.0.1:9876/proxy/v1
```

Der API-Schlüssel kann **ein beliebiger Wert** sein — AiDog leitet mit Ihrem konfigurierten echten Schlüssel weiter.

### Schritt 3: Verifizieren

```bash
curl http://127.0.0.1:9876/proxy/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer any-value" \
  -d '{"model": "gpt-4o", "messages": [{"role": "user", "content": "Hello!"}]}'
```

Eine normale KI-Antwort bedeutet, dass das Setup abgeschlossen ist. Anfragen werden automatisch geroutet, gemessen und protokolliert.

## Client-Integration im Detail

### Claude Code

AiDog bietet eine vollständige Integration unter **„Einstellungen → Claude Code"** (Modell/Berechtigungen/Sandbox/Plugins/Hooks/StatusLine bearbeiten, Ein-Klick-Import/Export).

**Option 1: Umgebungsvariablen (am schnellsten)**

```bash
export ANTHROPIC_BASE_URL="http://127.0.0.1:9876"
export ANTHROPIC_API_KEY="any-value"
claude
```

**Option 2: Ein-Klick-Konfigurationsexport**

Klicken Sie in „Einstellungen → Claude Code" auf „Zu Claude Code exportieren"; AiDog schreibt `~/.claude.json`:

```json
{ "apiBaseUrl": "http://127.0.0.1:9876" }
```

**Per-Gruppen-Isolation** — klicken Sie auf „Gruppeneinstellungen synchronisieren", um unabhängige Konfigurationen pro Gruppe zu erzeugen (`~/.aidog/settings.<gruppenname>.json`); die „Claude"-Schaltfläche der Gruppenkarte kopiert den Startbefehl.

### OpenAI Codex

Bearbeiten Sie `~/.codex/config.toml` (oder im Tab „Einstellungen → Codex"):

```toml
[provider]
name = "openai"
base_url = "http://127.0.0.1:9876/proxy/v1"
api_key = "any-value"

[model]
name = "o3"
```

> Codex verwendet die Responses-API (`/v1/responses`); AiDog erkennt und routet sie automatisch.

### Jeder OpenAI-/Anthropic-kompatible Client

Richten Sie den `base_url` / `OPENAI_API_BASE` / `ANTHROPIC_BASE_URL` des Clients auf `http://127.0.0.1:9876/proxy/v1` und verwenden Sie einen beliebigen Wert als Schlüssel.

> 🔐 **Gruppenauthentifizierung** — Setzen Sie den **Gruppennamen** als Schlüssel in der Proxy-Adresse; AiDog routet per Bearer-Token zur passenden Gruppe: `Authorization: Bearer <gruppenname>`.

![AiDog — Einstellungen](screenshots/settings.png)

## Aus dem Quellcode bauen

```bash
# Klonen
git clone https://github.com/lazygophers/aidog.git
cd aidog

# Abhängigkeiten installieren
yarn install

# Dev-Modus
yarn tauri dev

# Produktions-Build
yarn tauri build
```

**Voraussetzungen** — Node.js ≥ 18, Yarn 4.x, Rust-Toolchain (rustup), Tauri CLI, BS-spezifische Systemabhängigkeiten (siehe [Tauri Prerequisites](https://v2.tauri.app/start/prerequisites/)).

## Tech-Stack

| Schicht | Technologie |
| --- | --- |
| Desktop-Framework | Tauri 2.0 |
| Frontend | React 19 + TypeScript + Vite |
| Backend | Rust + Axum-Proxy + SQLite-Speicher |
| Docs | Rspress (8-sprachige Seite) |
| Build | Yarn 4 + Vite + cargo |

## Dokumentation

Vollständige Docs-Site 👉 <https://lazygophers.github.io/aidog/de/>

| Thema | Link |
| --- | --- |
| Schnellstart | [/getting-started/quick-start](https://lazygophers.github.io/aidog/de/getting-started/quick-start) |
| Installationsanleitung | [/getting-started/installation](https://lazygophers.github.io/aidog/de/getting-started/installation) |
| Plattform-Protokolle | [/platforms/protocols](https://lazygophers.github.io/aidog/de/platforms/protocols) |
| Gruppen & Routing | [/groups/routing-rules](https://lazygophers.github.io/aidog/de/groups/routing-rules) |
| Intelligentes Scheduling | [/groups/scheduling](https://lazygophers.github.io/aidog/de/groups/scheduling) |
| Codex-Integration | [/proxy/codex-integration](https://lazygophers.github.io/aidog/de/proxy/codex-integration) |
| Middleware-Regeln | [/middleware](https://lazygophers.github.io/aidog/de/middleware/) |
| Nutzungsstatistiken & Preise | [/stats/usage-stats](https://lazygophers.github.io/aidog/de/stats/usage-stats) |
| API-Referenz | [/api/api-reference](https://lazygophers.github.io/aidog/de/api/api-reference) |

## Mehrsprachiges README

| Sprache | Datei |
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

## Danksagung

[![LINUX DO](https://ld.xh.do/ld-badge.svg)](https://linux.do)

Danke an die [LINUX DO](https://linux.do)-Community.

## Lizenz

[GNU AGPL-3.0-or-later](LICENSE) © AiDog

Dieses Projekt steht unter der GNU Affero General Public License v3 oder höher. Wenn Sie diese Software ändern und als Netzwerkdienst anbieten, müssen Sie den entsprechenden Quellcode den Nutzern zugänglich machen.

<div align="center">

# AiDog

**Verwalte KI-Plattformen auf deinem Rechner und leite Client-Anfragen an die Plattformen deiner Wahl weiter.**

Eine Desktop-App mit lokalem Proxy, Gruppen-Routing, Modellzuordnung, Anfrageprotokollen und Nutzungsschätzungen.

[Dokumentation](https://lazygophers.github.io/aidog/en/) · [Neueste Version](https://github.com/lazygophers/aidog/releases/latest) · [Probleme melden](https://github.com/lazygophers/aidog/issues)

[简体中文](README.md) · [English](README.en.md) · [Français](README.fr.md) · Deutsch · [Русский](README.ru.md) · [العربية](README.ar.md) · [Español](README.es.md) · [日本語](README.ja.md)

</div>

AiDog betreibt einen Proxy-Dienst auf deinem Rechner. Du speicherst Upstream-Plattformen und API-Schlüssel in AiDog und verbindest anschließend Claude Code, Codex, pi oder andere kompatible Clients mit dem lokalen Proxy. Anfragen werden weiterhin an den ausgewählten Upstream gesendet; der lokale Betrieb bedeutet nicht, dass Anfragen auf deinem Rechner bleiben.

## Für wen

- Du nutzt mehrere KI-Dienste und möchtest Plattform-Zugangsdaten und Modelle in einer App verwalten.
- Verschiedene Clients sollen unterschiedliche Gruppen, Modellzuordnungen oder Plattformkandidaten verwenden.
- Du möchtest den Anfragestatus, die Nutzung, geschätzte Kosten und Routing-Ergebnisse einsehen.
- Claude Code, Codex oder pi sollen denselben lokalen Proxy-Einstiegspunkt nutzen.

## Funktionen

![AiDog Übersicht](screenshots/home-light.png)

![KI-Plattformverwaltung](screenshots/platforms-light.png)

*Illustrative Oberfläche mit Beispieldaten.*

### Lokaler Proxy und Routing

- Verwalte integrierte Plattform-Voreinstellungen und konfiguriere eigene Plattformen.
- Trenne Client-Anfragen durch Gruppen und ordne jeder Gruppe eigene Plattformen zu.
- Wähle Plattformkandidaten anhand von Modellzuordnung, Endpunktprotokoll und Verfügbarkeit aus.
- Slots mit mehreren Modellen unterstützen geordnete Modellketten; schlägt eine Anfrage fehl, werden nachfolgende Kandidaten versucht.
- Unterstützt Weiterleitung und Anpassung für OpenAI-, Anthropic-, Gemini- und weitere Protokolle; die tatsächlichen Möglichkeiten hängen vom Plattform-Endpunkt und Anfragetyp ab.
- Unterstützt Middleware-Regeln, zeitabhängige Modellkonfiguration und gesundheitsbasiertes Routing.

### Protokolle und Statistiken

- Zeige Proxy-Anfragen sowie separate Einträge für Tests, Kontostandsabfragen und Modelllisten-Anfragen an.
- Bewahre den Rohinhalt auf beiden Seiten einer Anfrage gemäß den Einstellungen auf oder lösche ihn; sensible Anfrage-Header werden maskiert.
- Schätze Kosten anhand der Antwortnutzung und integrierter Modellpreise. Schätzungen ersetzen nicht die Abrechnung des Upstreams.
- Zeige Plattformguthaben, Kontingente und Nutzungsstatistiken pro Gruppe an; diese Kennzahlen folgen unterschiedlichen Berechnungsmethoden und sind nicht direkt vergleichbar.

### Clients und Werkzeuge

- Verwalte Konfigurationen für Claude Code, Codex und pi und erstelle Verbindungsanweisungen für jede Gruppe.
- Verwalte MCP-Server, Skills, Benachrichtigungen und Systemeinstellungen.
- Optional kann ein Headless-Kernel ausgeführt und über eine auf lokalen Zugriff beschränkte Verwaltungs-API gesteuert werden.

## Installation

Lade das für dein System passende Installationspaket von [GitHub Releases](https://github.com/lazygophers/aidog/releases/latest) herunter. Verfügbare Pakete, Architekturen und Anforderungen pro System stehen auf der jeweiligen Release-Seite.

Nach dem Start lauscht der lokale Proxy standardmäßig auf `127.0.0.1:9890`. Standardmäßig wird nur die Loopback-Adresse verwendet; leite die Verwaltungs-API nicht ins öffentliche Internet weiter.

## Schnellstart

1. Füge unter „AI-Plattformen“ eine Plattform hinzu und trage Protokoll, Base URL und Upstream-API-Schlüssel ein.
2. Erstelle auf der Plattformseite eine Gruppe und füge ihr Plattformen hinzu.
3. Verwende im Client den für die Gruppe erzeugten Verbindungsbefehl oder richte die lokale Proxy-Adresse und Gruppen-Zugangsdaten gemäß der Client-Anleitung ein.
4. Sende eine Anfrage und prüfe Gruppe, Upstream-Plattform, Modell und Ergebnis im Proxy-Protokoll.

[Installationsanleitung](https://lazygophers.github.io/aidog/en/getting-started/installation) · [Erste Konfiguration](https://lazygophers.github.io/aidog/en/getting-started/first-config) · [Erste Anfrage prüfen](https://lazygophers.github.io/aidog/en/getting-started/first-request)

## Daten und Datenschutz

AiDog speichert Einstellungen und Datenbanken auf deinem Rechner. Proxy-Anfragen leiten notwendige Inhalte an den ausgewählten Upstream-Dienst weiter. Ob Anfrageprotokolle Anfrage- und Antwortkörper speichern, hängt von den Protokolleinstellungen ab; bei deaktivierter Rohinhaltsaufzeichnung können Metadaten wie Status, Modell und Nutzung weiterhin protokolliert werden. Siehe [Daten und Datenschutz](https://lazygophers.github.io/aidog/en/core-concepts/data-privacy).

## Aus dem Quellcode starten

Installiere zuerst die Werkzeuge und Systemabhängigkeiten gemäß den [offiziellen Tauri-Voraussetzungen](https://v2.tauri.app/start/prerequisites/).

```bash
git clone https://github.com/lazygophers/aidog.git
cd aidog
yarn install
yarn tauri dev
```

Installationspaket erstellen:

```bash
yarn tauri build
```

## Lizenz

AiDog steht unter der [GNU AGPL-3.0-or-later](LICENSE).

<div align="center">

# 🐕 AiDog

**Passerelle API IA unifiée**

Agrégation multi-plateforme · Routage intelligent · Analytique d'utilisation — une application de bureau multiplateforme pour gérer clés, requêtes et dépenses sur toutes vos plateformes IA

[![Documentation](https://img.shields.io/badge/docs-lazygophers.github.io/aidog-0a66c2?logo=sphinx&logoColor=white)](https://lazygophers.github.io/aidog/fr/)
[![GitHub Release](https://img.shields.io/github/v/release/lazygophers/aidog?logo=github)](https://github.com/lazygophers/aidog/releases)
[![License](https://img.shields.io/badge/license-MIT-blue)](#license)

[简体中文](README.md) · [English](README.en.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Русский](README.ru.md) · [العربية](README.ar.md) · [Español](README.es.md) · [日本語](README.ja.md)

</div>

> 📖 **Documentation complète** : <https://lazygophers.github.io/aidog/fr/>

AiDog est une passerelle API IA de bureau basée sur Tauri, qui unifie la gestion, le routage et la supervision des requêtes sur plus de 50 plateformes IA. Elle regroupe dans une seule application les clés API dispersées, les correspondances de modèles, l'équilibrage de charge et l'analytique d'utilisation — sans service backend, sans cloud, toutes les données stockées localement.

## ✨ Fonctionnalités

- **Agrégation multi-plateforme** — plus de 50 préréglages (Anthropic / OpenAI / DeepSeek / GLM / Kimi / MiniMax / Qwen, etc.), configuration en un clic
- **Groupes intelligents** — correspondance des requêtes par jeton Bearer / chemin ; routage Failover et Load Balance
- **Correspondance de modèles** — substitution transparente du nom de modèle (ex. `claude-sonnet-4` → `deepseek-chat`)
- **Conversion de protocole** — conversion bidirectionnelle entre les protocoles OpenAI Chat / Completions / Responses, Anthropic et Gemini
- **Équilibrage de charge & basculement** — relance automatique entre plateformes en cas d'échec, disjonction / gestion tri-état / backoff exponentiel
- **Suivi d'utilisation** — statistiques de jetons, estimation des coûts, requêtes de solde par plateforme, affichage du quota Coding Plan
- **Journalisation des requêtes** — trois niveaux de granularité (requête utilisateur / requête amont / résumé), chacun avec son commutateur et sa rétention
- **Moteur de règles middleware** — règles entrantes/sortantes : normalisation, surcharge, caviardage, injection, filtrage de mots sensibles, détection d'erreurs
- **Intégration d'assistants de code** — prise en charge native en un clic pour Claude Code, OpenAI Codex et autres assistants
- **i18n & thèmes** — 8 langues (dont arabe RTL), Liquid Glass et d'autres thèmes en modes clair/sombre

## 🚀 Démarrage rapide

### Téléchargement et installation

Récupérez l'installateur pour votre plateforme (macOS / Windows / Linux) depuis [GitHub Releases](https://github.com/lazygophers/aidog/releases).

Voir le [Guide d'installation](https://lazygophers.github.io/aidog/fr/getting-started/installation).

### Trois étapes

1. **Ajouter une plateforme** — saisissez la clé API et le point de terminaison d'une plateforme IA
2. **Configurer le proxy** — pointez l'URL de base de votre client vers le proxy local :
   ```
   http://127.0.0.1:9876/proxy/v1
   ```
3. **Commencer** — les requêtes sont routées, mesurées et journalisées automatiquement

Vérifiez avec curl :

```bash
curl http://127.0.0.1:9876/proxy/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer any-value" \
  -d '{"model": "gpt-4o", "messages": [{"role": "user", "content": "Hello!"}]}'
```

> 💡 La clé API dans l'URL du proxy peut être any-value — AiDog relaie avec votre vraie clé configurée.

📖 Tutoriel complet : [Démarrage rapide](https://lazygophers.github.io/aidog/fr/getting-started/quick-start).

## 🧩 Pile technique

| Couche | Technologie |
| --- | --- |
| Framework bureau | Tauri 2.0 |
| Frontend | React 19 + TypeScript + Vite |
| Backend | Rust + proxy Axum + stockage SQLite |
| Build | Yarn + Vite |

## 🛠️ Développement

```bash
yarn                          # installer les dépendances frontend
yarn tauri dev                # lancer l'app bureau (dev)
yarn build                    # build frontend (tsc && vite build)
cd src-tauri && cargo build   # build backend Rust
cd src-tauri && cargo clippy  # lint Rust (les warnings doivent être propres)
cd src-tauri && cargo test    # tests Rust
```

Prérequis : Node.js ≥ 18, Yarn 4.x, toolchain Rust, Tauri CLI.

## 📚 Documentation

Site de docs complet : <https://lazygophers.github.io/aidog>

- [Démarrage rapide](https://lazygophers.github.io/aidog/fr/getting-started/quick-start)
- [Protocoles de plateforme](https://lazygophers.github.io/aidog/fr/platforms/protocols)
- [Groupes & routage](https://lazygophers.github.io/aidog/fr/groups/routing-rules)
- [Intégration Codex](https://lazygophers.github.io/aidog/fr/proxy/codex-integration)
- [Statistiques & tarification](https://lazygophers.github.io/aidog/fr/stats/usage-stats)

## 🌍 Langues

| Langue | README |
| --- | --- |
| 简体中文 | [README.md](README.md) |
| English | [README.en.md](README.en.md) |
| Français | [README.fr.md](README.fr.md) |
| Deutsch | [README.de.md](README.de.md) |
| Русский | [README.ru.md](README.ru.md) |
| العربية | [README.ar.md](README.ar.md) |
| Español | [README.es.md](README.es.md) |
| 日本語 | [README.ja.md](README.ja.md) |

## IDE recommandé

[VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer).

## Licence

MIT

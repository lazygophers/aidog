<div align="center">

# 🐕 AiDog

**Passerelle API IA unifiée**

Application de bureau · Sans cloud · 50+ plateformes en un · Routage intelligent · Analytique d'usage

[![Documentation](https://img.shields.io/badge/docs-lazygophers.github.io/aidog-0a66c2?logo=sphinx&logoColor=white)](https://lazygophers.github.io/aidog/fr/)
[![GitHub Release](https://img.shields.io/github/v/release/lazygophers/aidog?logo=github&label=release)](https://github.com/lazygophers/aidog/releases/latest)
[![License](https://img.shields.io/badge/license-MIT-blue)](#license)
[![Platforms](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey?logo=tauri&logoColor=white)](https://github.com/lazygophers/aidog/releases/latest)

[简体中文](README.md) · [English](README.en.md) · `Français` · [Deutsch](README.de.md) · [Русский](README.ru.md) · [العربية](README.ar.md) · [Español](README.es.md) · [日本語](README.ja.md)

</div>

---

> 📖 **Documentation complète** : <https://lazygophers.github.io/aidog/fr/>

AiDog est une **passerelle API IA de bureau** basée sur Tauri. Elle unifie la gestion, le routage et la supervision des requêtes sur plus de 50 plateformes IA — en regroupant dans une seule application les clés API dispersées, les correspondances de modèles, l'équilibrage de charge, l'analytique d'usage et la configuration des assistants de code. Sans service backend, sans cloud ; toutes les données restent dans une base SQLite locale.

![AiDog — Tableau de bord](screenshots/dashboard.png)

## Ce qu'elle résout

| Votre douleur | Comment AiDog la traite |
| --- | --- |
| Clés API dispersées sur une douzaine de plateformes, pénible à basculer | **Agrégation multi-plateforme** — 50+ préréglages, gérer chaque clé au même endroit |
| Une plateforme tombe et tout votre flux s'arrête | **Basculement + équilibrage de charge** — relance auto, disjonction, scheduling entre plateformes |
| Claude Code / Codex / chaque client configuré séparément | **Intégration native des assistants de code** — export de config en un clic, tout le trafic via le proxy |
| Aucune idée de votre dépense mensuelle ni de la plateforme bientôt épuisée | **Suivi d'usage** — tokens + estimation de coût + solde + quota Coding Plan |
| Données à ne pas mettre dans le cloud ni chez des tiers | **Pur local** — proxy + base sur votre machine, zéro exfiltration |

## Fonctionnalités clés

### 🌐 Passerelle & routage
- **Agrégation multi-plateforme** — 50+ préréglages (Anthropic / OpenAI / DeepSeek / GLM / Kimi / MiniMax / Qwen / SiliconFlow / OpenRouter, etc.), configuration en un clic
- **Groupes intelligents** — correspondance par jeton Bearer / chemin ; Failover et Load Balance
- **Correspondance de modèles** — substitution transparente du nom de modèle (ex. `claude-sonnet-4` → `deepseek-chat`)
- **Conversion de protocole** — bidirectionnelle entre OpenAI Chat / Completions / Responses, Anthropic et Gemini
- **Disjonction & scheduling** — disjonction auto des plateformes anormales, gestion tri-état, backoff exponentiel, scheduling intelligent intra-groupe
- **Moteur de règles middleware** — règles entrantes/sortantes : normalisation, surcharge, caviardage, injection, filtrage de mots sensibles, détection d'erreurs, avec préréglages intégrés

### 📊 Suivi & statistiques
- **Suivi d'usage** — stats de tokens, estimation des coûts (synchro auto des prix + budget manuel)
- **Requêtes de solde** — solde de chaque plateforme en temps réel
- **Quota Coding Plan** — affichage et compte à rebours du quota Coding Plan DeepSeek / Kimi / GLM
- **Journaux de requêtes** — trois niveaux de granularité (requête utilisateur / requête amont / résumé), chacun avec commutateur et rétention indépendants

### 🤖 Intégration assistants de code
- **Claude Code** — intégration native : édition de config, import/export en un clic, scripts StatusLine, Hooks, synchro de config par groupe
- **OpenAI Codex** — intégration native : éditeur `~/.codex/config.toml`, routage auto de l'API Responses
- **Gestion MCP** — stockage centralisé en DB + activation par agent + scan & import + masquage des champs sensibles
- **Gestion Skills** — liste unifiée cross-platform basée sur npx + activation par élément
- **Notifications système** — annonces TTS / popup / boîte de réception + injection en un clic du hook Claude Code/Codex

### 🎨 Personnalisation
- **Système de thèmes** — 3 axes : 9 styles (Liquid Glass / Flat / Soft / Sharp / Aurora / Paper / Terminal / Bento / Sketchy) × 12 palettes nommées (Apple Blue / Nord / Dracula / Catppuccin / Gruvbox / Tokyo Night / One Dark / Material / GitHub / Night Owl, etc.) × modes clair/sombre
- **Internationalisation** — 8 langues (dont arabe RTL)
- **Import & export** — conteneur mono-fichier chiffré AES-256-GCM `.aidogx`, 7 périmètres avec résolution de conflit par élément
- **Tray + barre d'état** — actions rapides depuis le tray système + scripts de barre d'état personnalisables (Python + uv)

## Installation

### Configuration requise

| OS | Version minimale | Notes |
| --- | --- | --- |
| macOS | 12.0 (Monterey) | Intel + Apple Silicon |
| Windows | Windows 10 | x64 |
| Linux | x86_64 / aarch64 | Nécessite WebKit2GTK |

**Téléchargement** 👉 <https://github.com/lazygophers/aidog/releases/latest>

### macOS

1. Téléchargez le `.dmg` depuis [Releases Latest](https://github.com/lazygophers/aidog/releases/latest)
2. Double-cliquez pour l'ouvrir, glissez **AiDog** dans le dossier `Applications`
3. Au premier lancement, **clic droit** sur l'app → sélectionnez « Ouvrir » (contourner Gatekeeper — l'app n'est pas signée)

> ⚠️ Si le premier lancement indique « développeur non vérifié », allez dans `Réglages Système → Confidentialité et sécurité → Ouvrir quand même`.

### Windows

1. Téléchargez l'installateur `.msi` depuis [Releases Latest](https://github.com/lazygophers/aidog/releases/latest)
2. Double-cliquez l'installateur et suivez les invites
3. Si SmartScreen bloque, cliquez sur « Plus d'infos → Exécuter quand même »

### Linux

```bash
# Paquet DEB
sudo dpkg -i aidog_0.1.0_amd64.deb

# Ou AppImage
chmod +x aidog_0.1.0_amd64.AppImage
./aidog_0.1.0_amd64.AppImage
```

> Linux nécessite d'abord la dépendance WebKit2GTK : `sudo apt install libwebkit2gtk-4.1-dev` (Debian/Ubuntu).

### Premier lancement

Après installation, lancez AiDog — il va automatiquement :

1. Démarrer le serveur proxy local (par défaut `http://127.0.0.1:9876`)
2. Créer la base SQLite locale (`~/.aidog/aidog.db`)
3. Afficher l'interface principale et vous guider pour ajouter votre première plateforme

## Démarrage rapide (3 étapes)

### Étape 1 : Ajouter une plateforme

![AiDog — Ajouter une plateforme](screenshots/add-platform.png)

1. Cliquez sur **« Plateformes »** dans la navigation de gauche
2. Cliquez sur **« + Ajouter une plateforme »**
3. Renseignez : **Nom** (ex. `Mon OpenAI`), **Base URL** (ex. `https://api.openai.com/v1`, incluant le préfixe de version `/v1`), **API Key**
4. Enregistrer

> 💡 La Base URL inclut déjà le préfixe de version ; AiDog ajoute `/chat/completions` automatiquement — inutile de construire le chemin à la main.

### Étape 2 : Pointer le client vers le proxy

Dans l'app qui consomme les API IA, changez l'adresse API pour l'adresse proxy AiDog :

```
http://127.0.0.1:9876/proxy/v1
```

La clé API peut être **n'importe quelle valeur** — AiDog relaie avec votre vraie clé configurée.

### Étape 3 : Vérifier

```bash
curl http://127.0.0.1:9876/proxy/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer any-value" \
  -d '{"model": "gpt-4o", "messages": [{"role": "user", "content": "Hello!"}]}'
```

Une réponse IA normale signifie que la config est complète. Les requêtes sont routées, mesurées et journalisées automatiquement.

## Intégration client en détail

### Claude Code

AiDog fournit une intégration complète dans **« Réglages → Claude Code »** (édition modèle/permissions/sandbox/plugins/Hooks/StatusLine, import/export en un clic).

**Option 1 : Variables d'environnement (le plus rapide)**

```bash
export ANTHROPIC_BASE_URL="http://127.0.0.1:9876"
export ANTHROPIC_API_KEY="any-value"
claude
```

**Option 2 : Export de config en un clic**

Cliquez sur « Exporter vers Claude Code » dans « Réglages → Claude Code » ; AiDog écrit `~/.claude.json` :

```json
{ "apiBaseUrl": "http://127.0.0.1:9876" }
```

**Isolation par groupe** — cliquez sur « Synchro réglages groupe » pour générer des configs indépendantes par groupe (`~/.aidog/settings.<nom-groupe>.json`) ; le bouton « Claude » de la carte de groupe copie la commande de lancement.

### OpenAI Codex

Éditez `~/.codex/config.toml` (ou éditez dans l'onglet « Réglages → Codex ») :

```toml
[provider]
name = "openai"
base_url = "http://127.0.0.1:9876/proxy/v1"
api_key = "any-value"

[model]
name = "o3"
```

> Codex utilise l'API Responses (`/v1/responses`) ; AiDog la détecte et la route automatiquement.

### Tout client compatible OpenAI / Anthropic

Pointez le `base_url` / `OPENAI_API_BASE` / `ANTHROPIC_BASE_URL` du client vers `http://127.0.0.1:9876/proxy/v1` et utilisez n'importe quelle valeur comme clé.

> 🔐 **Authentification par groupe** — Mettez le **nom du groupe** comme clé dans l'adresse proxy ; AiDog route vers le groupe correspondant par jeton Bearer : `Authorization: Bearer <nom_groupe>`.

![AiDog — Réglages](screenshots/settings.png)

## Build depuis les sources

```bash
# Cloner
git clone https://github.com/lazygophers/aidog.git
cd aidog

# Installer les dépendances
yarn install

# Mode dev
yarn tauri dev

# Build production
yarn tauri build
```

**Prérequis** — Node.js ≥ 18, Yarn 4.x, toolchain Rust (rustup), Tauri CLI, dépendances système par OS (voir [Tauri Prerequisites](https://v2.tauri.app/start/prerequisites/)).

## Pile technique

| Couche | Technologie |
| --- | --- |
| Framework bureau | Tauri 2.0 |
| Frontend | React 19 + TypeScript + Vite |
| Backend | Rust + proxy Axum + stockage SQLite |
| Docs | Rspress (site en 8 langues) |
| Build | Yarn 4 + Vite + cargo |

## Documentation

Site de docs complet 👉 <https://lazygophers.github.io/aidog/fr/>

| Sujet | Lien |
| --- | --- |
| Démarrage rapide | [/getting-started/quick-start](https://lazygophers.github.io/aidog/fr/getting-started/quick-start) |
| Guide d'installation | [/getting-started/installation](https://lazygophers.github.io/aidog/fr/getting-started/installation) |
| Protocoles de plateforme | [/platforms/protocols](https://lazygophers.github.io/aidog/fr/platforms/protocols) |
| Groupes & routage | [/groups/routing-rules](https://lazygophers.github.io/aidog/fr/groups/routing-rules) |
| Scheduling intelligent | [/groups/scheduling](https://lazygophers.github.io/aidog/fr/groups/scheduling) |
| Intégration Codex | [/proxy/codex-integration](https://lazygophers.github.io/aidog/fr/proxy/codex-integration) |
| Règles middleware | [/middleware](https://lazygophers.github.io/aidog/fr/middleware/) |
| Stats & tarification | [/stats/usage-stats](https://lazygophers.github.io/aidog/fr/stats/usage-stats) |
| Référence API | [/api/api-reference](https://lazygophers.github.io/aidog/fr/api/api-reference) |

## README multilingue

| Langue | Fichier |
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

<div align="center">

# AiDog

**Gérez vos plateformes d'IA sur votre machine et transférez les requêtes de vos clients vers les plateformes de votre choix.**

Une application de bureau offrant un proxy local, le routage par groupes, le mappage de modèles, la journalisation des requêtes et l'estimation de la consommation.

[Documentation](https://lazygophers.github.io/aidog/en/) · [Dernière version](https://github.com/lazygophers/aidog/releases/latest) · [Signaler un problème](https://github.com/lazygophers/aidog/issues)

[简体中文](README.md) · [English](README.en.md) · Français · [Deutsch](README.de.md) · [Русский](README.ru.md) · [العربية](README.ar.md) · [Español](README.es.md) · [日本語](README.ja.md)

</div>

AiDog exécute un service proxy sur votre ordinateur. Vous enregistrez les plateformes amont et les clés API dans AiDog, puis vous connectez Claude Code, Codex, pi ou d'autres clients compatibles au proxy local. Les requêtes sont toujours envoyées vers l'amont sélectionné ; une exécution locale ne signifie pas que les requêtes restent sur votre machine.

## À qui ça s'adresse

- Vous utilisez plusieurs services d'IA et souhaitez gérer les identifiants et les modèles dans une seule application.
- Vous voulez que différents clients utilisent différents groupes, mappages de modèles ou plateformes candidates.
- Vous avez besoin de consulter l'état des requêtes, la consommation, les coûts estimés et les résultats de routage.
- Vous voulez que Claude Code, Codex ou pi partagent un même point d'entrée proxy local.

## Ce qu'il fait

### Proxy local et routage

- Gérez les préréglages de plateformes intégrés, et configurez des plateformes personnalisées.
- Isolez les requêtes des clients par groupes et associez à chaque groupe ses propres plateformes.
- Sélectionnez les plateformes candidates selon le mappage de modèles, le protocole du point de terminaison et la disponibilité.
- Les emplacements multi-modèles prennent en charge des chaînes de modèles ordonnées ; les candidates suivants sont essayés en cas d'échec d'une requête.
- Prend en charge le transfert et l'adaptation pour les protocoles OpenAI, Anthropic, Gemini et autres ; les capacités réelles dépendent du point de terminaison de la plateforme et du type de requête.
- Prend en charge les règles de middleware, la configuration des modèles par période et la planification de santé du routage.

### Journaux et statistiques

- Consultez les requêtes du proxy ainsi que les enregistrements distincts des tests, des requêtes de solde et des listes de modèles.
- Conservez ou effacez le contenu brut des deux côtés d'une requête selon les paramètres ; les en-têtes de requête sensibles sont masqués.
- Estimez les coûts à partir de la consommation des réponses et des prix de modèles intégrés. Les estimations ne remplacent pas la facturation de l'amont.
- Consultez les soldes des plateformes, les quotas et les statistiques d'utilisation par groupe ; ces indicateurs utilisent des méthodes de calcul différentes et ne sont pas directement équivalents.

### Clients et outils

- Gérez les configurations de Claude Code, Codex et pi, et générez les instructions de connexion par groupe.
- Gérez les serveurs MCP, les skills, les notifications et les paramètres système.
- Exécutez éventuellement un noyau headless, piloté via une API de gestion limitée à l'accès local.

## Installation

Téléchargez depuis [GitHub Releases](https://github.com/lazygophers/aidog/releases/latest) le paquet d'installation correspondant à votre système. Les paquets, architectures et prérequis disponibles par système sont indiqués sur la page de cette version.

Après le démarrage d'AiDog, le proxy local écoute par défaut sur `127.0.0.1:9890`. Seule l'adresse de bouclage locale est écoutée par défaut ; ne transférez pas l'API de gestion vers l'internet public.

## Démarrage rapide

1. Ajoutez une plateforme dans « Plateformes IA » et renseignez le protocole, la Base URL et la clé API amont.
2. Créez un groupe sur la page des plateformes et ajoutez-y des plateformes.
3. Dans votre client, utilisez la commande de connexion générée pour ce groupe, ou suivez le guide d'intégration du client pour configurer l'adresse du proxy local et les identifiants du groupe.
4. Envoyez une requête et vérifiez le groupe, la plateforme amont, le modèle et le résultat dans le journal du proxy.

[Guide d'installation](/en/getting-started/installation) · [Première configuration](/en/getting-started/first-config) · [Valider votre première requête](/en/getting-started/first-request)

## Données et confidentialité

AiDog stocke les paramètres et les bases de données sur votre machine. Les requêtes du proxy transfèrent le contenu nécessaire au service amont sélectionné. La conservation des corps de requête et de réponse dans les journaux dépend des paramètres de journalisation ; une fois l'enregistrement du contenu brut désactivé, les métadonnées telles que le statut, le modèle et la consommation restent enregistrables. Voir [Données et confidentialité](/en/core-concepts/data-privacy).

## Compiler depuis les sources

Installez d'abord les outils et dépendances système selon les [prérequis officiels de Tauri](https://v2.tauri.app/start/prerequisites/).

```bash
git clone https://github.com/lazygophers/aidog.git
cd aidog
yarn install
yarn tauri dev
```

Compiler un paquet d'installation :

```bash
yarn tauri build
```

## Licence

AiDog est distribué sous licence [GNU AGPL-3.0-or-later](LICENSE).

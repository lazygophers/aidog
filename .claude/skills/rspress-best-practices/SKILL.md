---
name: rspress-best-practices
description: Rspress best practices for config, CLI workflow, content organization, frontmatter, MDX, themes, i18n, search, static assets, deployment, and debugging. Use when writing, reviewing, or troubleshooting Rspress documentation sites.
---

# Rspress Best Practices

Apply these rules when writing or reviewing Rspress (v2) sites.

## Configuration

- Use `rspress.config.ts` and `defineConfig` from `@rspress/core`
- Set `root` explicitly when docs are not under the default `docs/` directory
- Keep site-wide settings such as `title`, `description`, `icon`, `logo`, `base`, and `lang` in config instead of repeating them in page files
- Prefer first-class Rspress options before custom theme code or low-level bundler overrides
- Keep custom theme code in a top-level `theme/` directory and import original theme pieces from `@rspress/core/theme-original`

## CLI

- Use `rspress dev` for local development
- Use `rspress build` for production output
- Use `rspress preview` only for local preview of the built site
- Use `rspress eject` only when CSS variables, class overrides, or layout wrapping cannot solve the customization

> 🔴 **CHECKPOINT — `rspress eject`**: ejection copies built-in source into the repo and creates a permanent upgrade-maintenance burden (hard to revert cleanly). Before running it, confirm with the user that CSS variables, BEM overrides, and Layout slots have all been ruled out. Do not eject unprompted.

## Docs Structure And Navigation

- Keep docs content under one clear docs root and group pages by topic or workflow, not by team ownership
- Use `_meta.json` or `_nav.json` to control sidebar and navigation labels/order instead of encoding order in filenames
- Put reusable MDX snippets or shared components in shared files instead of duplicating them across pages
- Keep landing pages concise and link to deeper task-oriented guides from them

## Writing And Frontmatter

- Add clear `title` and `description` frontmatter, and set `sidebar`, `outline`, `navbar`, or `footer` only when page defaults are not enough
- Use `pageType: home`, `doc`, `doc-wide`, `custom`, or `blank` intentionally based on layout needs
- Write task-first headings and short intros; avoid marketing-heavy copy in technical docs
- Prefer one topic per page and split overly long pages by workflow or feature area
- Keep code examples minimal, runnable, and version-accurate

## MDX And Components

- Use MDX for interactive docs and embedded components, but keep the main narrative understandable as plain markdown
- Prefer documented Rspress theme/runtime APIs over importing from internal source paths
- For app-wide UI or providers, use `globalUIComponents` or theme overrides instead of repeating imports in each page

## Theme And Styling

- Prefer CSS variables for brand colors, spacing, and surface styling
- Prefer BEM class overrides or `Layout` slots before ejecting built-in components
- In `theme/` files, keep `export * from '@rspress/core/theme-original'` unless intentionally replacing a named export
- Avoid full component ejection unless config, CSS, and wrapping cannot meet the requirement

## I18n, Search, And AI

- For multilingual sites, organize locale content under per-language directories and keep navigation mirrored where practical
- Keep descriptions and other frontmatter text in the same language as the page content
- Configure search intentionally: use local search for small or medium sites, and hosted search when scale or cross-version indexing requires it
- Enable `llms` or `ssgMd` only when the site benefits from machine-readable outputs, and keep descriptions accurate because those outputs surface page summaries

## Assets And Public Files

- Import source-managed images and components from docs/theme source when they belong to the content
- Use `public/` only for assets that must keep stable URL paths, such as favicons, social images, or download files
- Reference public assets by absolute site path and make sure they still work when `base` is set

## Plugins And Integration

- Prefer official Rspress plugins for search, preview, and API-doc scenarios before building custom solutions
- For component or library docs, use `@rspress/plugin-preview` and `@rspress/plugin-api-docgen` when interactive demos or API tables are needed
- Keep plugin usage explicit in config and remove unused plugins to reduce maintenance cost

## Build, Deploy, And Debugging

- Validate both `rspress dev` and `rspress build`; a page that works in dev can still fail during static generation
- Verify broken links, missing assets, and wrong `base` handling before deployment
- Keep generated output out of source control unless the hosting workflow explicitly requires committed artifacts
- When debugging content issues, inspect the resolved docs root, frontmatter, and theme overrides before assuming a bundler problem

## Debugging Decision Tree

Match the symptom to its branch and check the listed cause before assuming a bundler bug.

- **`dev` works but `build` fails** → SSG runs without a browser. Suspect client-only code (`window`, `document`, `localStorage`, browser-only libs) executing at module/render time. Move it behind an effect or render it via `globalUIComponents` instead of inline page imports.
- **Links return 404 in the built/deployed site** → check `base` config vs how links are written. With a non-root `base`, hand-written absolute paths (`/foo`) break; use relative doc links or prefix with `base`.
- **Assets (images, favicon, social image) 404 after deploy** → file is referenced by a root-absolute path but lives outside `public/`, or `base` isn't applied. Put stable-URL assets in `public/` and verify the path resolves with `base` set.
- **Sidebar/nav order or labels wrong** → it's driven by `_meta.json` / `_nav.json`, not filenames. Fix the meta file rather than renaming files.
- **A page renders blank or with wrong layout** → check the page's `pageType` frontmatter and any `navbar`/`sidebar`/`outline`/`footer` overrides before touching theme code.
- **Content/frontmatter not picked up** → inspect the resolved docs `root`, the frontmatter block, and theme overrides; a misresolved `root` silently drops files.
- **Custom theme component not applied** → confirm `theme/` imports use `@rspress/core/theme-original` (v2) and that the named export is actually re-exported in `theme/index.tsx`.

## Avoid (Anti-Patterns)

Consolidated blacklist — do not do these:

- Do **not** repeat site-wide settings (`title`, `description`, `base`, `lang`, `logo`) in page files; keep them in config.
- Do **not** encode sidebar/nav order in filenames; use `_meta.json` / `_nav.json`.
- Do **not** import from internal Rspress source paths; use documented theme/runtime APIs.
- Do **not** repeat the same imports in each page for app-wide UI/providers; use `globalUIComponents` or theme overrides.
- Do **not** eject a built-in component when config, CSS variables, BEM overrides, or Layout wrapping can meet the requirement.
- Do **not** put content assets in `public/`; reserve `public/` for stable-URL files (favicons, social images, downloads).
- Do **not** write root-absolute asset/link paths that break under a non-root `base`.
- Do **not** commit generated build output unless the hosting workflow explicitly requires it.
- Do **not** assume a bundler problem before checking resolved `root`, frontmatter, and theme overrides.
- Do **not** ship after validating only `rspress dev`; a dev-passing page can still fail static generation.

## Documentation

- For the latest Rspress docs, read https://rspress.rs/llms.txt
- Use the config and API docs when checking exact option names or current behavior

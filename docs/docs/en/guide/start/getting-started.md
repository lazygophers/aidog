---
description: Set up an Rspress project, run local development, build production output, and choose next docs topics.
---

# Getting started

## Project structure

This repository's documentation site has the following structure:

- `docs/` — The documentation source directory, configured via `root` in `rspress.config.ts`.
- `docs/docs/zh/_nav.json` — The top navigation bar configuration for the zh locale site.
- `docs/docs/en/guide/_meta.json` — The sidebar configuration for the guide section (each section has its own `_meta.json`).
- `docs/docs/public/` — Static assets directory (site logo and icons).
- `docs/theme/` — Optional custom theme directory; this site overrides the default theme here.
- `rspress.config.ts` — The Rspress configuration file.

## Development

Start the local development server:

```bash
npm run dev
```

:::tip

You can specify the port number or host with `--port` or `--host`, such as `rspress dev --port 8080 --host 0.0.0.0`.

:::

## Production build

Build the site for production:

```bash
npm run build
```

By default, Rspress will output to `doc_build` directory.

## Preview

Preview the production build locally:

```bash
npm run preview
```

## Next steps

- Learn how to use [MDX & React Components](/guide/use-mdx/components) in your docs.
- Learn about [Code Blocks](/guide/use-mdx/code-blocks/) syntax highlighting and line highlighting.
- Learn about [Custom Containers](/guide/use-mdx/container) for tips, warnings, and more.
- Explore the full [Rspress documentation](https://rspress.rs/) for advanced features.

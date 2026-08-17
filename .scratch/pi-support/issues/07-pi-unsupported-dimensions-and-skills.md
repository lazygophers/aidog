# 07 — 不支持维度的诚实展示 + Skills 常开徽标

**What to build:** 用户在 MCP、Hooks、通知、cc-switch 导入这些页面上，看到 pi 被明确标为「pi 不支持」并附一句为什么，而不是一片空白让人以为 aidog 坏了。在 Skills 页，pi 显示为常开的静态徽标而不是一个点了没反应的开关。

**为什么不支持**（详见 `docs/adr/0002-no-mcp-hooks-or-statusline-for-pi.md`）：pi 刻意不内置 MCP；pi 没有配置式 hooks，它的「hook」是 extension 里的 TypeScript 事件处理器，没有 aidog 能写的配置键；pi 没有 statusline 概念；上游 cc-switch 不支持 pi，无内容可导。

**Skills 为什么零工作**：`skills` CLI 把 skill 装进跨 harness 的公共目录，而 pi 原生扫该目录，所以全局装过的 skill pi 自动全见。pi 没有 per-skill 启停概念，所以那里只能是静态徽标 —— 做成开关就是在骗用户。

本票是纯展示与文案，不写任何 pi 配置。

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] MCP、Hooks/通知、cc-switch 导入三处对 pi 显式标注不支持，并给出一句原因
- [ ] 这些标注不是空状态、不是错误态，用户能看出是产品决定
- [ ] Skills 页 pi 显示为静态常开徽标，不可点击切换
- [ ] Skills 页对 pi 的说明点出「pi 扫公共 skill 目录，无需逐个启用」
- [ ] 新增文案 8 语言齐，`check-i18n` 绿
- [ ] `yarn test` / `yarn build` 绿

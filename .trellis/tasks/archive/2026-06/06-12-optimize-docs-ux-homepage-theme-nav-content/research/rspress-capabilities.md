# Rspress 2.x 能力 + 首页/主题/导航语法（research）

来源：https://rspress.rs/llms.txt + /guide/basic/home-page.md（2026-06-12 WebFetch）

## 首页 frontmatter（pageType: home）

```yaml
---
pageType: home
hero:
  name: AiDog
  text: 一句话主张
  tagline: 副标题
  image:
    src: /screenshots/hero.png   # 注意 base /aidog/ 会自动加前缀；写 /screenshots/... 即可
    alt: AiDog
  actions:
    - theme: brand        # 主按钮（用品牌色）
      text: 快速开始
      link: /zh/getting-started/quick-start
    - theme: alt          # 次按钮
      text: GitHub
      link: https://github.com/lazygophers/aidog
features:
  - title: 多平台聚合
    details: ...
    icon: 🧩               # 可 emoji 或图片路径；反 slop 倾向用 SVG/图片或克制
    span: 1               # 网格跨列
    link: /zh/platforms/add-platform
---
```

- 每语言 `index.mdx` 各写一份，link 必须带 lang 前缀（/zh/.. /en/..）
- hero image 未就绪先省略或占位

## 主题定制

- CSS 变量覆盖品牌色：Rspress UI vars（/ui/vars.md）。品牌色变量族 `--rp-c-brand`、`--rp-c-brand-dark` 等
- 注入方式：
  - `builderConfig.html.tags` 加 `<link>`（现有 config 已用 tags 加 meta）
  - 或 globalStyles 插件 API（plugin-api：传样式文件绝对路径）
- 自定义主题：`/guide/basic/custom-theme.md`（slot 包裹 / eject 内置组件），shadcn/ui + Tailwind v3/v4 支持

## 导航 / 侧栏

- `themeConfig.nav`：顶部导航栏（数组，item: text/link/activeMatch，或 dir）
- `_nav.json` + `_meta.json`：约定式自动导航/侧栏
- i18n：`useI18n` hook + i18n.json 文本数据；nav 文案走 i18n key

## 搜索

- 内置全文搜索默认开启
- searchHooks 自定义关键词处理/过滤/数据源（/guide/advanced/custom-search.md）
- 可选 Algolia（@rspress/plugin-algolia）/ Typesense

## 布局组件（可选增强）

- Banner（顶部通知条）、HomeLayout/HomeHero/HomeFeature（首页 slot）
- DocFooter（编辑链接 / 更新时间 / 上下页）、LastUpdated、EditLink
- Layout/Root slot props 做自定义扩展

## 踩坑提醒（继承 docs-rspress-build memory）

- Node 26 下 SSG 静默挂起 → docs/mise.toml 钉 node 20
- `defineConfig` 从 `rspress/config` 导入（非 core）
- favicon/图片路径基于 `root: docs` → 实际在 `docs/docs/public/`
- 版本钉死 `2.0.0-beta.21`

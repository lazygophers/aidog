# PRD — 文档站 UX 优化（首页 / 主题 / 导航 / 内容）

## 目标

把 aidog Rspress 文档站从「纯 markdown 列表」升级为有品牌识别度、Stripe 式明亮专业风格的产品文档站。覆盖布局、人机交互、UI/UX、首页、搜索、内容五个维度，7 语言一致。

## 背景

- 站点：`docs/`（Rspress 2.0.0-beta.21，7 语言 zh/en/ja/fr/de/ar/es，`base: /aidog/`）
- 现状痛点：
  - 首页是纯 markdown（`# AiDog` + 列表），未用 `pageType: home` hero/feature 布局
  - 无顶部 nav 导航栏，只有 _meta.json 侧栏
  - 无品牌色 / 字体定制，搜索用内置默认
  - 截图「待补充」，无产品视觉

## Brand Spec（继承自 app `src/themes/liquidGlass.ts`）

> AiDog 自家产品，品牌资产 = app 主题，不另造。

### 核心资产
- **Logo**：`docs/docs/public/logo.svg`（当前「AD」深底圆角方块，basic；本任务可升级为更精致版本）
- **产品截图**：本任务采集真实 app 界面（Platforms / Groups / Logs / Stats / Settings），用户授权「我帮你截」→ 实施阶段跑 app 采集

### 色板（light 为主，Stripe 式明亮）
- Primary / accent: `#007AFF`（hover `#0056CC`，subtle `rgba(0,122,255,0.1)`）
- 深色模式 accent: `#4A9EFF`
- 背景: `#f0f0f3`（base）/ 白玻璃面
- 文字: `rgba(0,0,0,0.88)` / `0.5` / `0.3`

### 视觉签名
- Liquid Glass：半透明毛玻璃面板、内发光边、深度阴影 `--shadow-lg`
- 圆角体系：sm 10 / md 14 / lg 20 / xl 28
- 渐变 accent，缓动 `250ms cubic-bezier(0.4,0,0.2,1)`

### 反 slop 约束
- 禁紫渐变、emoji 堆砌图标、SVG 手画产品图
- 截图用真实界面，无则诚实占位「截图待补」
- accent 单一蓝色贯穿，不发明新色

## 交付物（单 task 串行，共享 rspress.config.ts + 主题，不拆并行）

### D1 — 品牌主题层
- 新建 `docs/docs/public/styles.css`（或 theme 目录），注入 CSS 变量覆盖 Rspress `--rp-c-brand*` = `#007AFF`，字体、圆角、玻璃质感
- config 经 `builderConfig.html.tags` 或 globalStyles 引入
- 验证：build 产物 brand 色生效，亮/暗双模式正常

### D2 — 首页 hero/feature 改造（7 语言）
- 7 个 `index.mdx` → `pageType: home` frontmatter：hero（name/text/tagline/image/actions 双 CTA：快速开始 + GitHub）+ features 网格卡（多平台聚合 / 智能路由 / 用量统计 / 日志 / Codex / 多协议 / 主题 / 托盘）
- hero image = 产品截图（D4 产出，未就绪先占位）
- 验证：7 语言首页渲染一致，CTA 链接正确（带 `/aidog/` base + lang 前缀）

### D3 — 导航 / 侧栏结构
- `themeConfig.nav` 加顶部导航栏（指南 / 平台 / 分组 / GitHub），7 语言走 i18n
- 侧栏 _meta.json label 复核、必要时加 section header
- 验证：nav 7 语言文案正确，侧栏分组清晰

### D4 — 内容深化 + 真实截图
- 跑 app（`yarn dev` 前端 / `yarn tauri dev`），采集真实界面截图，存 `docs/docs/public/screenshots/`
- 首页 hero + 关键文档页（platforms/add-platform、stats/usage-stats、logs/viewing）嵌图
- index.mdx「截图待补充」替换为真图
- 内容查漏补缺（feature 描述、quick-start 流程）
- 验证：截图真实（非 mockup），关键页有配图

### D5（低优）— 搜索
- 内置全文搜索已启用；确认 7 语言索引正常，必要时配 searchHooks
- 验证：build 后搜索可用

## MVP 范围

D1 + D2 + D3 必做（品牌 + 首页 + 导航是质感主体）。D4 截图依赖 app 可跑，尽力采集，不可跑则诚实占位。D5 仅验证不深做。

## 验证（全局）

- `cd docs && yarn build` EXIT=0，产物 `doc_build/`
- 本地 `yarn dev`（或 preview）肉眼过首页 + 2 内页，亮/暗模式
- 7 语言首页 + nav 文案齐全无缺漏
- 资源路径带 `/aidog/` 前缀（GitHub Pages 子路径）

## 失败处理

- app 跑不起来 → D4 截图诚实占位，标注「截图待补」，不阻塞 D1-D3
- Rspress 2.x beta 行为异常 → 查 rspress-best-practices / rspress-custom-theme skill，降级到 config 原生能力

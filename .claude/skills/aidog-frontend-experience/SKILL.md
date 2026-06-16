---
name: aidog-frontend-experience
description: aidog（Tauri+React 桌面应用）前端体验优化——UI 视觉、布局、人机交互三合一。落地 Liquid Glass 风格、7 语言 i18n+阿拉伯 RTL、无路由本地 state 导航 + navGuard 离页拦截、formatters/shared 组件复用、主题 light/dark 双变量。视觉重构阶段调用 huashu-design skill 取设计品味。触发词：UI 优化、界面优化、视觉、好看、布局、对齐、间距、交互、体验、liquid glass、玻璃拟态、RTL、暗色、主题、组件复用、设计。
when_to_use: 改 aidog 前端 UI/布局/交互；新增页面或组件想保持风格一致；觉得某页"难用/不好看/挤"；做暗色或 RTL 适配；统一数值格式或抽公共组件时
---

# aidog 前端体验优化

aidog 是 Tauri 2.0 + React 19 + TS 桌面应用。本 skill 把「UI 视觉 / 布局 / 人机交互」三个相邻域合并，因为在 aidog 里它们共用同一套基建（主题变量、shared 组件、navGuard、i18n）。改前端体验时按本流程走，避免新代码偏离既有风格、漏 RTL、漏离页拦截、重复造格式化函数。

## 何时用

- 改某页 UI / 觉得「不好看、挤、乱」要重排版。
- 新增页面 / 组件，想一开始就贴齐 Liquid Glass + 主题变量。
- 做暗色或阿拉伯 RTL 适配。
- 交互体验差（无反馈、误触、离页丢数据）。

## aidog 前端硬约束（动手前必读）

| 域 | 约束 | 单一事实源 |
|---|---|---|
| 主题 | 每主题 light + dark 两组 CSS 变量，禁裸 hex；变量名精确（`--accent-subtle` 非 soft），禁 `rgba(255,255,255)` fallback | `src/themes/*.ts`（liquidGlass/nord/dracula/catppuccin/solarized） |
| 风格 | Liquid Glass（玻璃拟态：半透明 + 模糊 + 描边） | `src/themes/liquidGlass.ts` |
| 数值格式 | 统一走 `utils/formatters.ts`，**禁页内重复定义** formatNumber / 金额 / 百分比 | `src/utils/formatters.ts` |
| 展示组件 | 卡片/统计/余额条复用 shared | `src/components/shared/`（CompactCard / StatChip / BalanceBar / colorScale / usageColor） |
| 导航 | 无 react-router，导航 = `App.tsx` 侧栏 + `AppSettings.tsx` tab 的本地 state | `src/App.tsx` / `src/pages/AppSettings.tsx` |
| 离页拦截 | 禁原生 `confirm`/`beforeunload`（破坏 Tauri）；用 navGuard 注册表 | `src/utils/navGuard.ts` |
| i18n | 7 语言（zh-CN/en-US/ar-SA/fr-FR/de-DE/ru-RU/ja-JP），文案禁硬编码，走 `t()` | `src/locales/`，门禁 `yarn check:i18n` |
| RTL | ar-SA 是 RTL，方向相关样式（margin/padding/对齐/图标方向）必须逻辑属性或 `[dir]` 适配 | — |
| 拼音搜索 | 中文搜索走 `utils/pinyin.ts`，禁另写 | `src/utils/pinyin.ts` |

## 执行流程

### Step 1：定位 + 读现状（禁空想改）

1. 找到目标页/组件（`src/pages/` 或 `src/components/`）。注意巨石文件：`Platforms.tsx`(128KB)、`Groups.tsx`(46KB)、`Logs.tsx`(36KB) 改前先读相关段，别整文件重排。
2. grep 现有同类实现：要加格式化先 grep `formatters`；要加卡片先看 `components/shared`。**已有就复用，不新造**（见反例 #1）。
3. 确认涉及哪些主题——改色/间距必须同步该主题的 light + dark 两组变量。

### Step 2：取设计品味（视觉/重构类才需要）

纯视觉重构、「让它好看」、整页重排 → **调用 `huashu-design` skill** 取设计决策（层次、留白、对比、视觉重量）。
小改（改一个间距、补一个 hover 态）→ 跳过，直接 Step 3。

🔴 CHECKPOINT：整页视觉重构属高影响改动，改前把方案（截图/描述）给用户确认，再动手。禁直接重写整页。

### Step 3：实施（贴齐约束表）

- 颜色/间距 → 用主题 CSS 变量，不写裸值；同步 light+dark。
- 数字/金额/百分比 → `formatters.ts` 的函数；缺则加到 formatters 而非页内。
- 新文案 → 加 7 语言 key（至少 zh/en，其余可英文兜底但要有 key）。
- 方向相关样式 → 逻辑属性（`margin-inline-start` 等）或 `[dir="rtl"]` 适配。
- 离页有未保存数据 → 走 navGuard 注册，禁原生 confirm。

### Step 4：门禁验证（必跑）

```bash
yarn check:i18n     # i18n key 完整性（缺 key 会红）
yarn build          # tsc && vite build —— 类型 + 构建必须过
```

🔴 CHECKPOINT：`yarn build` 不过禁宣告完成。i18n 报缺 key 必须补齐，禁留裸 key（项目反复栽在这）。

## 失败模式编码（if-then）

| 触发 | 一线修复 | 仍失败兜底 |
|---|---|---|
| 改色后暗色模式错乱 | 检查是否只改了 light 组变量 | 两组变量都改；对照 `liquidGlass.ts` 的 dark 段 |
| ar-SA 下布局镜像错位 | 用逻辑属性替换物理 margin/padding | 加 `[dir="rtl"]` 专门覆盖；图标用 `scaleX(-1)` |
| `yarn check:i18n` 报缺 key | 7 个 locale 文件都补该 key | 跑 `scripts/check-i18n.mjs` 看具体哪个 locale 缺 |
| 离页数据丢失但没拦截 | 确认用了 navGuard 注册而非原生 confirm | 检查 `navGuard.ts` 注册表是否在卸载时注销 |
| 新组件与现有视觉不一致 | 改用 `components/shared` 现成组件 | 调 huashu-design 取一致性方案 |

## 反例黑名单（不要做）

1. ❌ 页内重新定义 `formatNumber` / 金额格式化 —— 必用 `formatters.ts`。
2. ❌ 写裸 hex / `rgba(255,255,255,…)` fallback —— 用主题变量。
3. ❌ 只改 light 不改 dark —— 两组必须同步。
4. ❌ 用原生 `confirm` / `beforeunload` 做离页拦截 —— 破坏 Tauri，必用 navGuard。
5. ❌ 新文案直接写中文字面量 —— 必走 `t()` + 7 语言 key。
6. ❌ 把 `Platforms.tsx` 这种巨石文件整体重排 —— 只改相关段。
7. ❌ 跳过 `yarn build` / `yarn check:i18n` 就说做完了。

## 相关

- 视觉品味：`huashu-design` skill
- 请求链路调试：`aidog-request-inspect` skill
- 性能问题：`aidog-perf-audit` agent
- 项目约定全文：根 `CLAUDE.md` 的「UI / i18n」节

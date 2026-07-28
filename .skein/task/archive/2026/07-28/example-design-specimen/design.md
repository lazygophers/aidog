# example/ 设计样例 — 详细设计

## 载体决策：单 HTML 文件自包含

用户诉求「样例文件」+「设计稿对齐」= 可便携、可分享、即时对比。

**选单 HTML**（非 React 页）：
- React 页需 `yarn dev` + vite 编译，不便携，设计稿对齐场景过重
- 单 HTML 自包含（内联 CSS + JS），浏览器直接打开，可邮件/IM 分享
- shadcn 组件用内联 CSS 仿 mono 风格（圆角/阴影/间距对齐 `src/components/ui/`），非引真组件（真组件需 React+tailwind 编译，设计稿不必）

**单文件双模 toggle**（非两文件）：
- `data-mode="light|dark"` 属性 + 顶部 toggle 按钮，JS 切换 `document.documentElement.dataset.mode`
- light/dark 两套 `:root[data-mode="..."]` CSS 变量块，即时对比
- 比 `light.html` + `dark.html` 两文件更便对比（设计稿核心诉求）

## 主题色真值源

内联 `src/themes/mono.ts` 的 light/dark 两套色变量（`--background`/`--foreground`/`--primary`/`--accent`/`--card`/`--popover`/`--border`/`--ring`/`--shadow-color` 等）+ `src/styles/globals.css` 别名映射（`--bg-surface`/`--bg-glass`/`--glass-edge`/`--text-secondary`/`--shadow-rgb` 等）+ `:root` 固定语义色（`--color-success/warning/danger/neutral` + 对应 `-bg`）。

签名色互换：light `--primary=#0087EB`（蓝主）/ dark `--primary=#FFD98A`（金主）— 双模 primary 不同色是 mono 签名。

## 结构（单文件分区）

```
example/design-specimen.html
├── <head> 内联全部 CSS
│   ├── :root 语义色（固定，light/dark 共享）
│   ├── :root[data-mode="light"] mono light 色变量 + 别名映射
│   ├── :root[data-mode="dark"]  mono dark 色变量 + 别名映射
│   ├── .glass / .glass-surface / .glass-elevated / .glass-highlight（玻璃层）
│   ├── @keyframes fadeIn/slideInLeft/pulseGlow/shimmer/statusPulse/spin/bgShimmer
│   ├── 组件样式（button/input/card/dialog/select/badge/alert/spinner/skeleton/progress/tabs/breadcrumb/table/accordion/switch/slider）
│   └── 图表样式（SVG 折线/环形/柱状 + 色阶条）
├── <body data-mode="dark">
│   ├── 顶栏：标题 + light/dark toggle 按钮
│   ├── §色板：primary/accent/secondary/muted/destructive/success/warning/danger/neutral swatches（前景+背景）
│   ├── §排版：h1-h4 / 正文 / muted / caption / 代码块
│   ├── §按钮：primary/secondary/ghost/destructive/outline + loading(spinner) + disabled + icon
│   ├── §表单：input(focus ring)/select/checkbox/radio/switch/slider/textarea
│   ├── §反馈：alert(success/warning/danger/info) + toast + spinner + skeleton + progress(bar+ring)
│   ├── §导航：tabs(active) + breadcrumb + dropdown menu
│   ├── §数据：glass card + badge(variants) + table + accordion
│   ├── §动效：fadeIn demo / slideInLeft demo / pulseGlow dot / shimmer skeleton / statusPulse / spinner / bgShimmer bg + .glass hover flow-border
│   └── §图表：SVG 折线（smoothPath 平滑曲线，cost trend 风格）+ 环形（progress ring）+ 柱状 + colorScale 色阶条（success/warning/danger/neutral）+ usageColor 用量速率条
└── <script> 内联：mode toggle + dropdown 开合 + accordion 折叠 + 动效触发按钮
```

## 图表（SVG 自绘，非 recharts）

项目 `src/utils/chart.ts` smoothPath 用 Catmull-Rom→三次贝塞尔。样例内联等价实现（纯 JS 生成 SVG path `d`），绘：
- **折线**：cost trend 风格（mock buckets），渐变填充区 + 平滑曲线 + 节点圆点
- **环形**：progress ring（stroke-dasharray 动画）
- **柱状**：日用量条形
- **色阶条**：colorScale 4 级（success/warning/danger/neutral）+ usageColor 用量速率（绿/黄/红渐变）

颜色全走 `var(--color-*)` 语义色，双模自适应。

## 动效（globals.css keyframes 全覆盖）

- `bgShimmer` 32s alternate（body 背景，dark 极淡金光晕）
- `fadeIn` 350ms（卡片入场）
- `slideInLeft` 300ms（侧栏/dropdown）
- `pulseGlow`（状态点脉冲）
- `shimmer` 1.4s（skeleton 加载）
- `statusPulse`（在线状态）
- `spin`（spinner/loading）
- `.glass:hover` border `color-mix(primary 28%)` + shadow-md（发光边框签名）

样例提供「触发」按钮（fadeIn/slideInLeft re-trigger）+ 持续动画（spinner/shimmer/pulseGlow 自动跑）。

## 不变量

- 单文件自包含，0 外部请求（禁 CDN / 禁 import / 禁 fetch）
- 双模 primary 互换签名色正确（light 蓝 dark 金）
- 玻璃 .glass hover 发光边框（flow-border 签名）双模可见
- 语义色 light/dark 共享（globals.css `:root` 固定，非随 mode 变）

## 取舍

- **内联 CSS 仿 shadcn 非引真组件**：设计稿要便携，真组件需 React+tailwind 编译。仿 mono 风格（radius-lg=20px / shadow-md / 间距 16-24px）对齐 `src/components/ui/` 视觉即可。
- **图表 SVG 自绘非引 chart.ts**：chart.ts 是 TS + 项目内依赖，样例内联等价 JS（smoothPath 算法移植）保自包含。
- **不引 mono.ts 原文**：TS 模块需编译。色值手工抄入 CSS 变量块（mono.ts 是真值源，抄写时核对）。
- **动效不全量参数化**：globals.css keyframes 原样内联，duration/timing 不改（保与生产一致）。

## 可能性分支（研究期留痕，不进正文/subtask）

- 若后续要交互式调色（设计稿调色板）：加 `<input type="color">` 绑 CSS 变量，实时改 primary/accent。触发条件：用户要「调色工具」非「静态样例」。
- 若要多主题对比（未来加第二主题）：单文件加 `data-theme` 维度，CSS 变量块按 theme×mode 矩阵。触发条件：项目引入第二主题。

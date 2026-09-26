# 图表库选型对比（research #29）

日期：2026-09-13 ｜ 分支：`research/chart-lib-selection` ｜ 前置：#24（允许第三方包，勿重开）

## 结论（先说）

**推荐：shadcn/ui Charts（底座 Recharts v3），承接 7/9 图表类型；热力图×2 与仪表盘用自研 DOM+CSS 组件补缺。**

一句话理由：它是唯一一个「主题 = 换 CSS 变量、RTL = 换 dir、React = 原生」的候选，与本仓库 Liquid Glass CSS 变量主题 + ar-SA RTL + React 19 栈零桥接成本；唯二的类型缺口（热力图、仪表盘）恰好是 #24 已内定「DOM+CSS 自绘」的两类，自研面积最小。

## 候选与出处

| 候选 | 说明 | 关键出处 |
|---|---|---|
| shadcn/ui Charts | Recharts v3 的复制粘贴式封装组件（`chart.tsx` 进 `components/ui/`，源码归我们所有，非运行时依赖） | https://ui.shadcn.com/docs/components/chart |
| Recharts 直接用 | 声明式 React 图表组件，SVG 渲染，D3 底层数学 | https://recharts.github.io/ |
| Apache ECharts | canvas/SVG 双渲染器，20+ 图表类型全家桶 | https://echarts.apache.org/ |
| DOM+CSS+canvas 自研 | 零依赖对照组（#24 之前的原方案） | 本仓库 `src/utils/chart.ts`（smoothPath 手写 SVG） |

## 对比矩阵

判据 = #29 原文：九类型覆盖 / Liquid Glass 主题适配（CSS 变量）/ RTL（ar-SA）/ bundle / React 集成 / 与 #24 兼容 / 维护活跃度。

| 判据 | shadcn Charts | Recharts 直接用 | Apache ECharts | DOM+CSS 自研 |
|---|---|---|---|---|
| 九类型覆盖 | 7/9（缺热力图×2、仪表盘） | 同左（同一底座） | 9/9 全原生 | 9/9（但每类都要写） |
| 主题（CSS 变量） | **原生契合**：`--chart-1..N` CSS 变量 + `ChartConfig.theme{light,dark}`，填充色写 `var(--color-KEY)`；SVG 在 DOM 里，浏览器自己解析 CSS 变量，切主题零 JS | 同左（Recharts 接受任意 CSS 变量字符串作 fill） | **需自建桥**：主题 = JS 对象（`registerTheme` / `option.color` 数组），canvas 绘制不认 CSS 变量；每次切主题要 `getComputedStyle` 读值 + 重建 option | 完全 CSS，无需桥 |
| RTL | 良好：SVG/tooltip 均为 DOM，跟随 `dir` + 逻辑属性；坐标轴需手动反转（`XAxis reversed`）——由公共层统一处理一次 | 同左 | **硬缺口**：无内建 RTL，workaround = 每个 `xAxis.inverse:true` + 外层 CSS；canvas/SVG 两渲染器 RTL 文本行为还不一致 | 完全可控 |
| bundle | recharts v3.10.1 ≈ 548.5 kB min / 144.1 kB gzip（bundlephobia，经搜索转述）；tree-shaking 受 victory-vendor（D3）拖累 | 同左（多省一层 chart.tsx≈几 kB） | 全量 `echarts.min.js` = 1,121,883 B min（jsDelivr CDN HEAD 实测）；按需导入可显著缩小但官方未给数字，gzip 后对比 推测: 两者半斤八两（都在 100–200 kB gzip 区间）；Tauri 本地应用不打进网络下载，此项权重最低 | 0 |
| React 集成 | 原生声明式组件，与 Radix/Tailwind v4 同栈；`components.json` 已配置，`npx shadcn@latest add chart` 即装 | 原生，但 tooltip/主题/色板全自己拼 | **无官方 React 绑定**：官方是框架无关 JS；社区 `echarts-for-react` 3.0.6（周下载 44 万）但 npm 页面显示长期未更新（转述），更稳的是自写 hook（init/setOption/dispose 手动管生命周期） | 自写 |
| #24 兼容 | 声明式 API ✓；公共层坐标轴 ✓（包一层统一 XAxis/YAxis）；tooltip 用其 ChartTooltip + 最近点吸附（官方 cursor 机制）✓；路径生长动画 Recharts 内建（Line/Area 挂载动画）✓ | 同左，tooltip/动画都自己拼 | 声明式要靠自封装 ✓可做；tooltip/动画内建但样式是 echarts 自己的皮肤体系，要重写成 Liquid Glass 得跟它的 DOM 结构搏斗；「以库的渲染方式为准」→ canvas ✓ | 与 #24 各条完全对齐 |
| 维护活跃度 | recharts 3.10.1（2026-07-25 发布，deps.dev），周下载 3150 万（npmjs），持续发版 | 同左 | echarts 6.1.0（npm latest，约 4 个月前发布），周下载 330 万（snyk），Apache 基金会 9 维护者 | 只看我们自己 |

九类型逐一核对（Recharts 官方动画指南列出的组件全集 = Area / Bar / ErrorBar / Funnel / Line / Scatter / Pie / Radar / RadialBar / Treemap，出处 https://recharts.github.io/en-US/guide/animations/）：

| 类型 | Recharts/shadcn | ECharts |
|---|---|---|
| 折线 | LineChart ✓ | series-line ✓ |
| 柱状 | BarChart ✓ | series-bar ✓ |
| 堆叠柱 | BarChart stacked ✓ | series-bar stack ✓ |
| 堆叠面积 | AreaChart stacked ✓ | series-line area+stack ✓ |
| 饼/环形 | PieChart（innerRadius 变环形）✓ | series-pie ✓ |
| 散点 | ScatterChart ✓ | series-scatter ✓ |
| 热力图 ×2 | ✗ 无原生 | series-heatmap ✓（https://echarts.apache.org/en/option.html#series-heatmap） |
| 仪表盘 | ✗ 无原生（RadialBarChart 半圆可近似） | series-gauge ✓（https://echarts.apache.org/en/option.html#series-gauge） |

## 为什么选 shadcn Charts（关键理由展开）

1. **主题桥为零**。仓库主题 = 每主题 light+dark 两组 CSS 变量（`src/themes/mono.ts`，单文件持 shaddn 语义 token + 结构变量）。shadcn Charts 官方主题机制就是同一套：`--chart-1..N` 变量 + `ChartConfig` 里 `theme:{light,dark}` 切换、系列色写 `var(--color-KEY)`（出处：https://ui.shadcn.com/docs/components/chart）。ECharts 的主题是 `registerTheme()` 注册的 JS 对象（出处：https://echarts.apache.org/handbook/en/concepts/style/），canvas 绘制不读 CSS 变量——8 主题 × light/dark = 每主题要么生成一份 JS 主题对象，要么写运行时 CSS 变量→JS 桥。这是一条长期维护的裂缝。
2. **Liquid Glass 语义一致**。SVG 渲染进 DOM → 背景 transparent、tooltip 是真 DOM 节点，可以挂 `backdrop-filter`、吃 CSS 变量、跟 PopoverCards 等现有玻璃面板同一视觉语言。ECharts canvas 是一块不透明位图，玻璃效果只能用它的 skin 系统模拟。
3. **RTL 路径明确**。官方文档有 RTL 专节并给了 Arabic 示例（出处同上 chart 文档页）；剩余工作（X 轴 reversed、tooltip 容器溢出）恰是 #24 已锁的「公共层统一处理」事项。ECharts 无内建 RTL（出处：apache/echarts#19609，https://github.com/apache/echarts/issues/19609；canvas/SVG 渲染器不一致：#21465）。
4. **不是锁死**。shadcn 官方明说「We do not wrap Recharts components. You have full control… you are not locked into an abstraction」（出处：chart 文档页）——chart.tsx 只是进 `src/components/ui/` 的自有源码，任何时候可以直接用裸 Recharts 逃生（自定义形状做热力图近似等）。
5. **缺口即最小自研面**。缺的两类 = 热力图、仪表盘，正是 #24 决策 2 里本来就规划「DOM+CSS」的类型：热力图 = CSS grid 色块矩阵（色阶用 `color-mix()` 从 CSS 变量派生），仪表盘 = SVG 弧 + 指针 或 conic-gradient。两个小组件，不引任何新依赖。
6. **装配成本 ≈ 0**。`components.json` 已配好（new-york / cssVariables:true / aliases @/components/ui），一条 `npx shadcn@latest add chart` 装完；动画升级可抄 Evil Charts（Recharts+shadcn+Tailwind 动画图表块，复制粘贴，https://evilcharts.com）。

## 落选者：什么条件下才对

- **ECharts 才对**：当仪表盘/热力图需求变得重且复杂（多仪表联动、日历热力图、大数据量 canvas 加速、brush/dataZoom 交互、地图/3D），或图表数量从 9 种膨胀到全家桶时。届时引 `echarts/core` 按需导入（出处：https://echarts.apache.org/handbook/en/basics/import/），自写 React hook 封装，接受 JS 主题桥 + RTL workaround。混合共存也成立：ECharts 只挂热力图/仪表盘两个页面，其余仍走 shadcn——但**当前 9 类需求下没必要**。
- **Recharts 直接用（不套 shadcn）才对**：当我们想砍掉 chart.tsx 这层自有源码、tooltip/主题 token 全自管时。但它不省依赖（recharts 照装）、省的只是 ~200 行自有封装，还丢了现成的 CSS 变量主题契约和 RTL 基线——不划算。
- **DOM+CSS+canvas 自研才对**：当应用要发 web 且首屏体积/零依赖是硬指标，或团队对第三方图表库有审计约束时。Tauri 本地应用 bundle 打进二进制，这条判据不成立（#29 原文也把 bundle 列为低权重项）；且自研 = 两套渲染心智 + 9 类图表全部手写坐标轴/tooltip/动画，工作量远超补两个缺口组件。

## 需要新增的落地面（给实现票）

1. `src/themes/*.ts` 每主题补 `--chart-1..5`（light+dark 两组）——mono.ts 现无任何 `--chart-*` token（grep 已核）。
2. `npx shadcn@latest add chart` → `src/components/ui/chart.tsx` + `pnpm/npm add recharts`。
3. 热力图/仪表盘两个 DOM+CSS 自研组件，色值从 CSS 变量派生。
4. 旧三处迁移删除：`src/components/shared/CostTrendChart.tsx`（utils/chart.ts smoothPath 手写 SVG）、`src/pages/Home.tsx` 小时柱状、`src/components/PopoverCards.tsx` 消费曲线。
5. 公共层：统一 XAxis/YAxis（nice-ticks、时间轴走 utils/formatters.ts、双轴、RTL 反转）+ 单一 tooltip（最近点吸附 + 悬浮卡）。

## 数据可信度备注

- recharts 体积 548.5 kB min / 144.1 kB gzip：bundlephobia 数字经搜索结果转述（API 429 无法直取）；量级与官方 dist 一致，精确值 需要：bundlephobia 恢复后直取。
- ECharts gzip 体积未测（CDN HEAD 只有 min 未压缩值）；按 JS 库常规压缩比 推测: 300–350 kB gzip。按需导入后体积无官方数字，仅官方定性表述「substantially decrease the bundle size」（import 手册）。
- echarts-for-react 停更时长：npm 页面「last publish 4…」截断转述，精确日期 需要：npm registry 直接查询。

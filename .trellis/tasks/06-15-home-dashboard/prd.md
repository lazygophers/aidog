# 新增首页·总览仪表盘

## 需求（用户确认）
新增「首页」= **总览仪表盘**，一眼看全：代理状态/端口 + 今日用量·费用·请求数 + 分组/平台速览·余额 + 快捷操作。**直接实现进 React app**（复用现有主题/组件，风格必与现有 UI 一致）。**作侧栏首项 + 应用启动默认落地**（替代当前默认 platforms）。

> 设计原则（huashu-design）：从 aidog **现有设计系统长出来**（Liquid Glass + CSS 变量 + 共享组件），**不套通用风格库、不堆 AI slop**（禁紫渐变/emoji 图标/假数据/装饰性 icon）。真实数据 only，无数据留诚实空态。单一 accent 贯穿。Stats 页管深度分析，首页管「一眼概览 + 入口」，不与 Stats 重复堆图表。

## 现状（已核）
- 导航 `src/App.tsx` NAV 数组（:24 起）：platforms/groups/stats/logs/notifications/skills/mcp/settings/about；启动默认落 platforms。无 home。
- 共享组件 `src/components/shared/`：CompactCard / StatChip / BalanceBar / usageColor / colorScale + formatters（`src/utils/formatters.ts`）。
- 主题：Liquid Glass，CSS 变量（`--bg-floating`/`--accent`/`--text-secondary`/`--radius-*`/glass-surface/glass-elevated 等），`src/themes/`。
- 数据 API（全现成）：
  - `proxyApi.status()`→bool running；`proxyApi.getSettings()`→{port}。代理 base_url=`http://127.0.0.1:<port>/proxy`（复用 Groups 刚加的复制模式）。
  - `trayApi.todayStats()`→TodayStats（今日 cost/tokens/cache_rate/requests）。
  - `popoverApi.platformToday()`→TodayPlatformStat[]（各平台今日用量）。
  - `statsApi.query({start,end,...})`→{overview:StatsOverview}（可取 today 范围更全指标，参考 Stats.tsx:121 用法）。
  - `groupDetailApi.list()` / `platformApi.list()`；平台 `est_balance_remaining` 求和 = 总余额（BalanceBar）。

## 实现
### 页面 `src/pages/Home.tsx`（新建）
布局（自上而下，glass-surface 分区，复用共享组件）：
1. **Hero/状态条**：代理运行状态（running 绿 / stopped 灰，复用 popover 的状态色语义）+ 端口 + **复制代理 base_url 按钮**（CopyButton 模式，`http://127.0.0.1:<port>/proxy`）。可含品牌名/版本（about_info 可选）。
2. **今日概览**：一行 StatChip × 4 — 今日费用 / token / 请求数 / 缓存率（trayApi.todayStats，金额用 usageColor 语义色 + formatters 格式化）。无数据 → 诚实空态（「今日暂无请求」），不编造。
3. **分组/平台速览**：分组数 / 平台数（启用·熔断态可选）+ **总余额 BalanceBar**（platforms est_balance_remaining 求和）+ 平台今日用量 top N（CompactCard，popoverApi.platformToday）。
4. **快捷操作**：按钮卡 — 添加平台(跳 platforms) / 查看统计(跳 stats) / 看日志(跳 logs) / 复制代理地址。用 App 的 onNavigate 跳转（Home 接 `onNavigate` prop）。
- 加载用 Promise.all 并行拉数据 + loading 骨架；失败 catch 不崩（各区独立兜底）。
- 数值统一走 `utils/formatters.ts`，颜色走 usageColor，禁页内重复定义。

### 导航接入 `src/App.tsx`
- NAV 数组**最前**插 `{ id: "home", icon: "home", labelKey: "nav.home" }`。
- 启动默认页 platforms → **home**（找到初始 state/默认 nav 值改之）。
- 路由渲染加 `{effectiveNav === "home" && <Home onNavigate={handleNavigate} />}`（参考现有页面渲染分支 + About 的挂法）。
- Home 需要的跳转复用现有 `handleNavigate`。

### 图标 + i18n
- icons 加 `home` 图标（`src/components/icons` 加一个简洁 house/dashboard SVG，风格随现有 icon set，线性 stroke）。
- i18n `nav.home`（+ 首页内文案：今日费用/token/请求/缓存率/分组/平台/总余额/快捷操作/添加平台/查看统计/复制代理地址/暂无请求 等）8 locale 全补（zh-CN/en-US/ar-SA/fr-FR/de-DE/ru-RU/ja-JP/es-ES）；加 key 后 Counter 查重。品牌名保留不翻译。

## 验收
- `yarn build`（tsc+vite）+ `yarn check:i18n` 过；locale 无重复 key。
- 后端无改动（全复用现成 command）；若未动 src-tauri 免 cargo。
- 行为：启动落首页；侧栏首项「首页」；状态条显代理运行/端口 + 复制 base_url 生效；今日 4 指标真实（无数据空态）；分组/平台速览 + 总余额；快捷操作跳对应页。
- 风格：与现有页一致（Liquid Glass + 共享组件 + 单 accent），无 slop（无紫渐变/emoji 图标/假数据/装饰 icon 堆砌）。
- 不破坏现有导航/其它页。

## 失败处理
- 某 API 失败 → 该区诚实空态/占位，不整页崩。
- 默认落地改动影响 navGuard/初始 state → 找准初始 nav 来源改，确认刷新/启动都落 home。
- 数据与 Stats 重复堆图 → 首页只做「概览+入口」，深度图表留 Stats。
- 门禁红修到绿；卡住标 `需要:`。

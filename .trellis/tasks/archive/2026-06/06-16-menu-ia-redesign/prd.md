# 菜单与导航信息架构重构

## 决策（用户已选）
- 交互：**A 分组折叠** — 侧栏分 5 section，每个 section 可折叠（chevron 切换显隐）
- 跨页路径：**全做** — Home 快捷卡 + 平台→日志 + 分组→统计

## 现状
`App.tsx` BASE_NAV 8 扁平顶级 + settings 可展开树（10 子页×5 组）。Sidebar 已支持 NavItem.children 展开 + child.group 子分区，但**顶级无 section 分组**。

## 目标 IA
```
概览  [home]
代理  [platforms, groups]
观测  [stats, logs, notifications]
扩展  [skills, mcp]
系统  [settings(保留子树), about]
```
- section header：小标题 + chevron，折叠/展开该 section 项
- 单项 section（概览）仍显 header 保一致
- settings 在「系统」section 内保留自身子树展开（两级折叠可共存）
- logs/notifications 条件隐藏不变（log/notif 开关 off 时整 section 项移除；section 仅剩 1 项也照常）

## 导航带上下文（navContext）
跨页快捷跳转需带筛选参数。机制：
- `handleNavigate(id, context?)`：setActiveNav + setNavContext(context ?? {})
- `type NavContext = { platformId?, platformName?, groupId?, groupName?, model? }`
- App 持 navContext state，按目标页传 prop：
  - `<Logs initialFilter={{ platformId, platformName }} />`
  - `<Stats initialFilter={{ group?, platform? } } />`
- `key={effectiveNav}` 切页强制重挂载 → 页面 mount 时读 initialFilter 初始化筛选 state（天然 clean，无需手动 clear）

## 三个快捷路径
1. **Home 快捷卡**：仪表盘加 4 卡片（平台/分组/统计/日志），点击 onNavigate 对应 id
2. **平台→日志**：Platforms 列表每行加「日志」图标按钮 → onNavigate("logs", { platformId, platformName })
3. **分组→统计**：Groups 列表每行加「统计」图标按钮 → onNavigate("stats", { groupId, groupName })

## 实现步骤
1. Sidebar：NavItem 加 `section?: string`；render 按 section 分组，header 可折叠（expandedSection state，默认全展）
2. App.tsx：BASE_NAV 每项加 section；handleNavigate 扩 context；navContext state + 传 prop
3. Logs.tsx：加 `initialFilter?` prop，mount 时设 platform 筛选
4. Stats.tsx：加 `initialFilter?` prop，mount 时设 group/platform 筛选
5. Home.tsx：快捷卡片区
6. Platforms.tsx：行内日志快捷按钮
7. Groups.tsx：行内统计快捷按钮
8. i18n ×8：nav.group.overview/proxy/observe/extension/system + home.quickXxx

## 验证
- 折叠某 section → 其项隐藏，活动项所在 section 自动展开
- Home 点「平台」卡 → 跳 platforms；点「日志」→ 跳 logs
- Platforms 行点日志图标 → logs 仅该平台记录
- Groups 行点统计图标 → stats 仅该分组
- 切回 logs/stats 无残留筛选（重挂载 clean）
- cargo/tsc/i18n check 全过

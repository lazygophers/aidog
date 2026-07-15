# 菜单 + 统计结构 (D)

## D1. 侧栏菜单完整层级

**定义**: `src/App.tsx:24-53` `BASE_NAV` 数组 (NavItem[])。无 react-router, 导航靠 `activeNav` state + `effectiveNav` 分发 (App.tsx:148-153)。

**section 分组** (侧栏渲染按 section 聚合, 推测: Sidebar 组件按 NavItem.section 分段):

| section (i18n key) | NavItem id | 页组件 |
|---|---|---|
| nav.section.overview | home | Home |
| nav.section.proxy | platforms | Platforms |
| nav.section.observe | stats / logs / notifications | Stats / Logs / Notifications |
| nav.section.extension | skills / mcp | Skills / Mcp |
| nav.section.system | settings (12 子 tab) / about | AppSettings / About |

**顶层 NavItem (无 children)**: home / platforms / stats / logs / notifications / skills / mcp / about + settings (有 children 但本身也是顶层入口)。

**隐藏机制** (App.tsx:142-146): logs 关 → 隐 logs; notifications 关 → 隐 notifications。新模块若需条件隐藏, 同模式 filter。

## D2. "AI 平台统计" 对应

用户原话"AI平台统计"对应 **stats 页** (id="stats", i18n `nav.stats` = "使用统计" zh-Hans / "Usage Statistics" en, 见各 locale json `nav.stats` key)。**注**: 字面无"AI平台统计", 用户的称呼推测指 stats 页 (平台维度的使用统计)。platforms 页 = "AI 平台" (nav.platforms)。

stats 与 platforms **同级** (都在 BASE_NAV 顶层, 不同 section: stats 在 observe, platforms 在 proxy)。

## D3. Stats.tsx 数据源 + 聚合维度

`src/pages/Stats.tsx`:
- API: `statsApi.query` (line 150-151), 封装 invoke `query_stats` (后端 query_stats.rs)。
- 聚合维度 (`groupBy` state, line 115): `"platform" | "model" | "group"` 三选。
- 过滤: filter_platform / filter_group / filter_model (line 144 + 298 模型搜索), available_models 来自 proxy_log 实际记录 (line 195-197)。
- 时间范围: range.start/end + prevR 对比 (line 150-151), granularity minute (line 165)。
- 数据流: proxy_log → stats_agg_hourly (预聚合) → query_stats SQL GROUP BY。

**platform 维度** (query_stats.rs:184-190): `GROUP BY platform_id`, Stats.tsx:316 追加"无平台"(platform_id=0) 桶。
**group 维度**: GROUP BY group_key (group_name)。
**model 维度**: GROUP BY actual_model (query_stats.rs:458)。

## D4. 新模块菜单挂点

**挂同级 stats**: 在 BASE_NAV 加一个顶层 NavItem (App.tsx:24-53 数组), 例:
```ts
{ id: "cpa", icon: "cpa", labelKey: "nav.cpa", section: "nav.section.proxy" /* 或新 section */ },
```
+ App.tsx:155-183 渲染分支加 `{effectiveNav === "cpa" && <CpaModule />}`。

**section 归属选项** (不拍板):
- 归 `nav.section.proxy` (与 platforms 同段, 强调"也是代理平台")
- 归 `nav.section.observe` (与 stats 同段, 若重统计)
- 新建 section (如 nav.section.cpa, 完全独立)

**i18n**: 8 语言文件加 `nav.cpa` + section key (若新建)。

**条件隐藏**: 若新模块可选启用, 同 logs/notifications filter 模式 (App.tsx:142-146)。

## D5. "AI 平台页可添加本模块配置" 的入口

用户需求: Platforms 页可引新模块配置。当前 PlatformEditForm 入口模式 (CpaImportModal 寄生新建态, B6) 可复用: 新模块独立菜单为主入口, Platforms 页可放次要入口 (如"从 X 模块导入"按钮)。具体入口形态属设计决策, 不拍板。

## D6. 新模块独立数据表 vs 复用 platform 表

PRD 需求"独立数据表"。当前 platform 表 (schema_early.rs:13-60) 是所有平台通用表, cpa-* 复用之。新模块独立表意味着:
- 新建 `cpa_*` 表 (schema migration 新增)。
- 新建独立 db 模块 (如 `gateway/db/cpa.rs`)。
- 路由层 (candidates.rs / selection.rs) 需扩展从新表拉候选 (当前只从 platform 表)。
- proxy_log 需新字段或复用 platform_id (若复用, platform_id 命名空间冲突 — 推测: 需 `source_type` 区分 platform 表 vs cpa 表)。

**"内部平台类型(模型不可选, 从 provider 继承)"**: 新 Protocol 变体 (如 `internal`), 模型选择逻辑 (resolve_effective_models in candidates.rs) 需加"继承 provider 模型"分支。provider 概念当前不存在 (推测: 指新模块配置的 upstream provider, 模型列表从 provider 拉, 非用户配)。

**关键约束**: 独立表 + 独立路由 = router/proxy 层需感知第二候选源 (platform 表 + cpa 表), 当前 select_candidates 只查 platform 表。这是新模块最大的架构改动点, 非"加个菜单"量级。

## 需用户裁

1. 新模块菜单 section 归属 (D4)。
2. "模型从 provider 继承"的 provider 语义 (D6, 新概念需定义)。
3. 独立路由候选源如何与现有 platform 表候选融合 (D6)。

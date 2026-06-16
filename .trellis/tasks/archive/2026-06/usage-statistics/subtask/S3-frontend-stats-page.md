---
id: S3
slug: frontend-stats-page
deliverable: D2, D3
parent-task: usage-statistics
status: planned
execution-layer: main
isolation: none
depends-on: [S2]
blocks: []
estimated-tokens: 8000
---

# S3 · 前端统计页面 + 导航 + i18n

## 目标

实现统计页面组件，接入后端 API，添加侧栏导航入口和 7 语言 i18n 翻译。

## 产出

- `src/pages/Stats.tsx`: 完整统计页面
- `src/App.tsx`: 新增 Stats 路由 + 导航项
- `src/services/api.ts`: 新增 stats API 调用
- `src/locales/*.json` (7 文件): 新增统计相关翻译 key

## 验证

```bash
npx tsc --noEmit
```

期望输出: 无类型错误

手动验证: 页面加载 → 筛选切换 → 数据刷新

## 资源

- 独占文件: `src/pages/Stats.tsx`, `src/locales/*.json`
- 共享文件: `src/App.tsx`, `src/services/api.ts`

## 依赖

| 上游 | 需要的产出 | 等待方式 |
| --- | --- | --- |
| S2 | stats_query Tauri command 可调用 | cargo check 通过 |

## 执行细节

1. **api.ts**: 新增 `statsApi.query(params)` 封装 `invoke("stats_query", params)`
2. **Stats.tsx 页面结构**:
   - 顶部筛选栏: 时间范围 (今天/7天/30天/自定义) + 分组下拉 + 模型下拉 + 粒度 (小时/天)
   - 指标卡片区: 总请求 / 成功率 / 总 Token / 平均延迟 (glass-surface 卡片)
   - 趋势图区: 纯 CSS bar chart 或 SVG 绘制时间序列
   - 分布表格: 按 group_by 维度的排行榜
3. **App.tsx**: 新增 `nav.stats` 导航项 + `Stats` 组件渲染
4. **i18n**: 所有 locale 新增:
   - `nav.stats` / `page.stats`
   - `stats.totalRequests` / `stats.successRate` / `stats.totalTokens` / `stats.avgLatency`
   - `stats.granularity` / `stats.hourly` / `stats.daily`
   - `stats.timeRange` / `stats.today` / `stats.last7d` / `stats.last30d` / `stats.custom`
   - `stats.groupBy` / `stats.byPlatform` / `stats.byModel` / `stats.byGroup`
   - `stats.dimension` / `stats.requests` / `stats.tokens` / `stats.duration`

## 回滚

```bash
git checkout -- src/pages/Stats.tsx src/App.tsx src/services/api.ts src/locales/
```

## 风险

| 风险 | 影响 | 缓解 |
| --- | --- | --- |
| 图表渲染复杂 | 开发耗时 | 用简单 SVG bar chart，后续优化 |
| 大量数据前端卡顿 | 交互差 | 后端分页/限制返回行数 |

## 历史

- 2026-06-10: created

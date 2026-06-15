# 首页请求趋势改「最近24小时」

## 需求
首页「请求趋势 · 今日」→「请求趋势 · 最近24小时」。不仅改文案，**数据窗口也改**：从「今日 0 点→now」改为「now-24h → now」滚动窗口（hourly 24 桶）。

## 现状
- `src/pages/Home.tsx`：趋势取数 `statsApi.query({start: 今日0点, end: now, granularity:"hourly"})`（~:93，dayStart.setHours(0,0,0,0)）。
- 标题 i18n key `home.trendTitle`（值含「今日」）+ 顶部 `home.trendPeak/trendTotal`，8 locale。

## 实现
1. **取数窗口**：start 从「今日 0 点」改为 `now - 24*3600*1000`（滚动最近 24 小时），end=now，granularity 保持 hourly。其余取数/曲线/空态逻辑不变。
2. **文案**：`home.trendTitle` 值 8 locale 改「请求趋势 · 最近24小时」对应翻译（en "Request Trend · Last 24h"、ja「リクエスト推移 · 直近24時間」等，各 locale 地道译；品牌名/数字格式不变）。`trendPeak/trendTotal/trendEmpty` 语义仍适用，按需微调措辞（如空态「最近24小时暂无请求」）。
3. x 轴小时标注：滚动窗口跨昨天-今天，time_bucket 仍含小时，沿用现标注即可（必要时确认不串位）。

## 约束/范围
- **只改 `src/pages/Home.tsx` + i18n locale（8 个）**。⚠️ 并发会话在改 api.ts/Stats.tsx/mcp.rs/lib.rs——**禁碰** api.ts/Stats.tsx/src-tauri。提交**路径限定** `git add src/pages/Home.tsx src/locales/*.json`，禁 git add -A。
- 加/改 i18n 后 Counter 查重无新增重复。

## 验收
- `yarn build` + `yarn check:i18n` 过；locale 无新增重复 key。
- `git diff --name-only` 仅 Home.tsx + locale（不含 api.ts/Stats.tsx/src-tauri）。
- 行为：标题显「最近24小时」；趋势数据为 now-24h 滚动窗口（非今日 0 点起）；曲线/空态/刷新不变。

## 失败处理
- statsApi.query 是否支持任意 start（非自然日边界）→ 应支持（granularity hourly 按时间桶聚合）；若后端按自然日切桶导致首尾桶不齐，接受（首页概览，非精确）；异常标 `需要:`。
- 门禁红修到绿；卡住标 `需要:`。

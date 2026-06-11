# PRD: 平台 quota 展示区分 + 刷新

## 需求（已确认）
1. **样式区分**：coding plan/余额（quota）展示与 usage 使用信息**区分样式 + 不同位置**（当前两者都 StatBadge、仅分两行 marginTop 4/6，视觉雷同难分辨）
2. **coding plan 刷新** + 3. **余额刷新** → **统一 quota 刷新**（一个动作刷 balance+coding_plan，复用 quotaApi.query）
   - 刷新 UI：**badge 旁内联小图标**

## 现状
- Platforms.tsx 卡片：usage badge（:1636 tokens/cost/cache/ok）+ quota badge（:1645-1662 balance 💳 / coding plan 🪙），均 StatBadge，紧邻两行
- quotaApi.query(baseUrl, apiKey)（api.ts:545）合查返 PlatformQuota{balance, coding_plan}
- 启动批量加载 quotaMap（:871 区）；无手动刷新

## 设计决策
- **区分**：quota 区独立视觉分组 —— 与 usage 明显分开（加分组标签如「额度」+ 不同 badge 风格/色调/边框 + 位置上拉开间距或独立区域），Liquid Glass 风格
- **刷新**：quota 分组标签/区域旁加内联刷新小图标（↻），点击调 quotaApi.query 刷新该平台 quotaMap[p.id] + loading 态 + 错误 toast
- 纯前端（quotaApi.query 已存在，无后端改动）

## 涉及面
- src/pages/Platforms.tsx：quota 渲染区（:1645-1662）样式区分 + 刷新图标 + 刷新 handler + per-platform loading state

## 验收标准
- coding plan/余额 与 usage 视觉/位置明显区分
- quota 区内联刷新图标，点击刷新 balance+coding_plan + loading + 错误提示
- tsc 0 / yarn build

## 注意（多窗口并行）
别窗口正改 Platforms pricing 领域（PricingTab.tsx/api.ts pricing）。本 task 改 Platforms.tsx 卡片 quota 区，commit 仅限该区，避免卷入别窗口未提交改动。

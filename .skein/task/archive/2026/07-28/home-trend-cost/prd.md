# 首页趋势加价格曲线 — PRD

## 目标
- Home.tsx「请求趋势·今日」SVG 双曲线(请求数+tokens)升级为三曲线, 加 cost(花费)第3条
- tooltip 加 cost 行 + 环比变化%
- 图例加「花费」项
- 8 语言 i18n key home.trendCost 同步

## 边界
- 范围内: src/pages/Home.tsx + src/locales/*.json (8 语言)
- 范围外: 后端 (total_cost 字段 StatsBucket 已返, 零改)
- 约束: 第3色用 --color-warning (萤火虫暖色, 与 accent/info 区分); cost 归一化独立 (costPeak)

## 验收标准
- [ ] tsc 0 err / test 281 pass / build OK
- [ ] 趋势图第3条 cost 曲线可见, 色区分清晰
- [ ] tooltip hover 显示 cost 值(formatCostUsd) + 环比%
- [ ] 图例3项 (请求数/Tokens/花费)
- [ ] check-i18n 0 缺译 (8 语言 trendCost key 全)
## 索引
- 详细设计: 单文件微改, inline

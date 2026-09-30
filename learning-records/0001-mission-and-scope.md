---
name: mission-and-scope
date: 2026-09-28
---

# 0001 · 教学目标与范围确认

**Mission 定稿**（ask-ui aidog-20260928071456-ab75）：学会 aidog 全部路由机制，达到能讲给别人/写文档的水平。节奏=一节全景大图课；group/platform/模型概念已熟，无需铺垫。

**关键取舍**：
- 讲解视角偏「表达」（概念准确、图示为主），不是偏配置决策或日志排查——后续课程素材按可复述性组织，而不是按决策树。
- 第 1 课已覆盖全部机制（5 模式 + 6 闸门 + 熔断三态 + 模型三级联 + 叠加规则），无遗漏分支被刻意裁掉；深度待定的只有重试链的失败分类细则（scheduling.rs 失败分类表），留作第 2 课候选。
- 用户已懂的概念（group/platform/模型）不再解释；「有效权重」「EMA」这类半新词在 lesson 内联一句话解释。

**教学素材真值源**：`src-tauri/crates/aidog_core/src/gateway/router/`（全部文件带中文 doc 注释）。文档已存在但极简：`docs/docs/zh/core-concepts/groups-routing.mdx` 仅 10 行——写作时不应把它当讲义，当从源码重新出发。

## 2026-09-28 补记：第 2 课（health_aware）与 least_latency 删除

- 第 2 课交付：lessons/0002-health-aware.html。核心教学点：health_aware 当前实现与 load_balance 同一排序分支（candidates.rs:377-382 并列 match），熔断准入门对所有模式生效；它是全局默认模式（settings.rs default_routing_mode="health_aware"）。两个易混点写进课程：probe 桶≠半开探测、熔断全空回退透传。
- 用户同轮拍板删除 least_latency 路由模式（产品级，非仅课程），迁移 20260928-04 存量分组转 load_balance。第 1 课已同步为 4 种模式。

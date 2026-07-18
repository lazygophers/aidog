---
title: coding plan 订阅制平台普遍无公开用量查询 API
layer: recall
category: domain
keywords: [coding-plan,quota,upstream-api,degrade,custom-quota-script]
source: coding-plan-utilization-calib-fix
authored-by: skein-spec
created: 1784417181
status: active
related: []
updated: 1784417181
---

bailian/qianfan/xiaomi/compshare 等 coding plan 订阅制平台上游均无公开程序化用量查询 REST API (仅控制台页面看剩余请求次数), 且 ToS 明文禁套餐 key 用于非编程工具的 API 自动化调用。新增此类平台 quota handler 前先按「无上游 API, 走 custom-quota-script 兜底」预设, 别默认能建内置 handler。已支持的 kimi/glm/minimax 是少数例外。

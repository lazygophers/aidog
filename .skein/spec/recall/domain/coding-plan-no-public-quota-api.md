---
name: coding-plan-no-public-quota-api
title: coding-plan-no-public-quota-api
layer: recall
category: domain
keywords: [coding-plan, quota, public, 端点, glm_coding]
created: 1725080438
inclusion: auto
---


bailian/qianfan/xiaomi/compshare 等 coding plan 订阅制平台上游均无公开程序化用量查询 REST API (仅控制台页面看剩余请求次数), 且 ToS 明文禁套餐 key 用于非编程工具的 API 自动化调用。新增此类平台 quota handler 前先按「无上游 API, 走 custom-quota-script 兜底」预设, 别默认能建内置 handler。已支持的 kimi/glm/minimax 是少数例外。

---


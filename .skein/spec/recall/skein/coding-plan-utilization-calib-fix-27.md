---
title: task 查重: 同模块非重复, 先看 PRD 边界互引
layer: recall
category: skein
keywords: [skein,dedup,task-boundary,prd]
source: coding-plan-utilization-calib-fix
authored-by: skein-spec
created: 1784417181
status: active
related: []
updated: 1784417181
---

dedup/查重判定重叠维度前, MUST 先看两 task 的 PRD 边界条款是否已显式互相引用切割 (如双向标注对方 task id + 代码入口物理隔离)。命中即为分立强证据, 禁只凭「都碰同一模块/函数名」这类粗粒度标签合并。实证: coding-plan-utilization-calib-fix (改 query_quota_inner 内置 dispatch) vs custom-quota-script (全新 query_custom_quota 独立 entry) 同碰 quota 但 PRD 双向切割, 分立。

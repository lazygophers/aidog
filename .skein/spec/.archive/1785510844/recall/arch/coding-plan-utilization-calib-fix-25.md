---
title: coding plan 校准链路 base_url 真值源 = endpoint 级
layer: recall
category: arch
keywords: [coding-plan,base_url,quota,calibration,finish,est_coding_plan]
source: coding-plan-utilization-calib-fix
authored-by: skein-spec
created: 1784417180
status: active
related: []
updated: 1784417180
---

coding plan 平台 preset 平台级 base_url 恒为 None (真 base_url 在 endpoints 内)。finish/estimate 校准链路 (spawn_estimate/StreamEstCtx) 取 base_url MUST 从 forward 内解析的 endpoint base_url (target_base_url, forward.rs 与 coding_plan flag 同源同 scope) 传入, 禁传 route.platform.base_url (恒空致 query_quota dispatch 子串匹配失败, est_coding_plan 永不填充, tiers 空白)。反直觉点: 平台级 base_url 空是设计如此, 非 bug。

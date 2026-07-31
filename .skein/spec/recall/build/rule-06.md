---
title: rule-06
layer: recall
category: build
keywords: []
source: -
authored-by: skein-spec
created: 1784995466
status: active
related: []
updated: 1784995466
---
---
# converter 5×5 与 endpoint 选择解耦

## MUST 硬约束

converter 双向转（source→wire 请求 + wire→source 响应）与 endpoint 选择解耦：
- endpoint 层负责选哪个协议 endpoint（可跨协议回退）
- converter 层负责怎么转（已支持 5×5 全互转）
- 两层独立：endpoint 选哪个，converter 怎么转，互不依赖

## 反例

- ❌ 误判：endpoint 层限制只许选同协议 → converter 能力已就绪，endpoint 无需自我限制
- ❌ 误修：新增协议先修 endpoint 回退 → converter 未就绪 → 回退开不了

## 适用

- 所有新增 wire protocol 的变更
- endpoint 跨协议回退扩展
- converter 双向转换能力验证

## 关联


## 案例

- endpoint-cross-protocol-fallback task：converter 5×5 已就绪，endpoint 层开回退即释放全链路
- 新增协议只需加 converter parse/render + endpoint 自动回降级（无需 endpoint 层改动）
[[rule-55]]
[[rule-07]]

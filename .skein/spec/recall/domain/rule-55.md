---
title: endpoint 跨协议回退分层
layer: recall
category: domain
keywords: []
source: -
authored-by: skein-spec
created: 1784995459
status: active
related: []
updated: 1784995459
---
---
# endpoint 跨协议回退分层

## 触发场景

- 普通平台 endpoint 选择时协议不匹配（如 anthropic 入站 + 仅 openai endpoint）
- select_endpoint_for_protocol 步骤 4 跨协议回退判定

## 陷阱-正解

**陷阱**: 误以为跨协议回退可应用于所有平台类型，或回退优先级混乱。

**正解**: 普通平台步骤 4 泛化为三级回退（同协议 > openai > 任意非 source endpoint），coding 平台不动（步骤 1/2 保持 401 防护）。

## 分层不变量

- 回退仅在普通平台生效：普通平台允许跨协议回退（降低 502 率）
- coding 平台永不落非 coding：步骤 1/2 严格限制，非 coding endpoint 永不 fallback（安全边界）
- coding 端点缺失仍 401，非跨协议回退问题

## 反例

- ❌ 误判：coding 平台也跨协议回退 → 破坏 401 防护
- ❌ 误修：只修普通平台回退，忘了 coding 平台保持不变 → 混淆两路径

## 适用

- endpoint.rs select_endpoint_for_protocol 修改
- 跨协议回退逻辑扩展
- coding vs 普通平台路径区分

## 关联


## 案例

- endpoint-cross-protocol-fallback task：普通平台步骤 4 泛化（同协议 > openai > 任意非 source），coding 平台不动
[[rule-06]] [[rule-07]]

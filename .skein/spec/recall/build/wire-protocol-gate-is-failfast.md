---
title: rule-07
layer: recall
category: build
keywords: []
source: -
authored-by: skein-spec
created: 1784995469
status: active
related: []
updated: 1784995469
---
---
# is_valid_wire_protocol gate 是 fail-fast 非修复点

## MUST 硬约束

is_valid_wire_protocol gate 触发（502）说明 endpoint 选择失败（matched_ep=None fallback platform_type），根因在 select_endpoint_for_protocol 而非 gate 本身。

- ❌ 误判：gate 缺白名单 → 降级 gate（加协议到白名单）
- ✅ 正修：修 endpoint 选择逻辑（修 select，让 matched_ep 非 None）

## 反例

- 只修白名单而未修 select → 新协议仍 502（根因未除）
- 误判为 endpoint 配置缺 protocol → 实际是 DB endpoints 字段空（preset 未加载）

## 适用

- 所有 502 route fail 场景
- is_valid_wire_protocol gate 触发
- endpoint 选择失败诊断

## 关联


## 案例

- converter-reasoning-content bug1：preset 未加载致 matched_ep=None，gate 502
- endpoint-cross-protocol-fallback task：endpoint 层跨协议回退修 select，非降级 gate
[[protocol-wire-str]] 

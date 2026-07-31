---
name: auto-disable-401-403-402
title: 平台自动禁用仅触发 401/403/402
layer: core
category: proxy
keywords: [auto-disable,401,403,402,stateless,throttle]
created: 1725080438
inclusion: auto
---

## 硬约则

平台自动禁用（auto_disabled）仅由三个 HTTP 状态码触发：**401 / 403 / 402**，**禁 429 触发**。

- **401/403**：凭据失效/禁止，永久故障，禁用平台至主人手动恢复
- **402**：账户余额不足，永久故障同上
- **429**：限流，临时故障，走熔断机制（自动探测恢复），禁长期禁用

## 触发条件

见 `crates/aidog_core/src/gateway/proxy/non_success.rs:68`：

```rust
if code == 401 || code == 403 || code == 402
```

## 禁用

❌ 429 触发 auto_disabled → 永久禁用平台，虽然是临时故障  
❌ 其他 4xx（如 400）触发 auto_disabled → 隐藏真实错误原因

## 关联

[[core/arch/mock-platform-bypasses-forward-pipeline]] [[http-client-no-env-proxy]]

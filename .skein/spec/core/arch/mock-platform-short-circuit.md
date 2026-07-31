---
name: mock-platform-short-circuit
title: Mock 平台绕开转发流水线短路
layer: core
category: arch
keywords: [mock,platform,short-circuit,proxy]
created: 1725080438
inclusion: auto
---

Mock 平台在转发流水线早期短路，不走真实上游请求逻辑。

- `handler.rs:412` `matches!(first.platform.platform_type, Protocol::Mock)` 短路
- `:418 return handle_mock(...)` 拦截点位于 CONNECT/请求解析之前
- 压测/容量验证禁用 mock，仅用非 mock 平台

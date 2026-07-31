---
name: resolve-price-now-ms
title: resolve_price 末位 now_ms 传值约定
layer: core
category: domain
keywords: [billing,pricing,cache,timestamp]
created: 1725080438
inclusion: auto
---

`resolve_price` 最后一个参数 `now_ms: i64` 为价表缓存的时间戳校验位。各调用点传值约定：

- 余额扣减、估算、计费：所有路径同一 `now_ms` 确保口径一致
- `gateway/db/model_price.rs:180-188` 函数签名
- 调用点：`billing.rs` / `estimate/db_ops.rs` / `platform_cmd/price.rs`

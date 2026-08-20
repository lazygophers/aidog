---
title: router
layer: recall
category: proxy
keywords: [router,platform,sole_platform,单启用平台,短路,路由优化]
status: active
---

## 单启用平台分组判定 (sole_platform)

## 判定口径契约

### 定义

**单启用平台分组** (`sole_platform`) 判定唯一真值源位于 `src-tauri/crates/aidog_core/src/gateway/router/mod.rs:98-107`。

两个分支定义：

1. **物理单平台**：组内平台总数 == 1
   - 无论 status 为何都短路（保「唯一平台 auto_disabled 仍必请求」的既有契约）
   - 见 `src-tauri/crates/aidog_core/src/gateway/router/test_candidates.rs`

2. **唯一启用平台**：组内 `status == Enabled` 的平台恰好 1 个
   - 用户只启用了一个，等效单平台分组
   - auto_disabled/disabled 其他平台被过滤

### 不参与判定的维度

与 status 正交的临时闸门**不参与本判定**，由各自路径处理：
- `expires_at` 过期
- `disable_during_peak` 高峰禁用
- 短路后 `handle_single_platform` 仍会做高峰禁用硬停

### 调用点

| 调用点 | 文件 | 行号 | 用途 |
|---|---|---|---|
| 路由阶段 0 | `gateway/router/candidates.rs` | 186 | 分流到 `handle_single_platform` |
| Group info 端点 | `gateway/proxy/group_info.rs` | 110-121 | 判定 `applicable` |

### 实现

```rust
pub(crate) fn sole_platform(gps: &[GroupPlatformDetail]) -> Option<&GroupPlatformDetail> {
    if gps.len() == 1 {
        return Some(&gps[0]);
    }
    let mut it = gps.iter().filter(|gp| gp.platform.status == PlatformStatus::Enabled);
    match (it.next(), it.next()) {
        (Some(only), None) => Some(only),
        _ => None,
    }
}
```

`(it.next(), it.next())` 双取模式避免 collect 分配，天然表达「恰好一个」。

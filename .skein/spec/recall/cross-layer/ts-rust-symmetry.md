---
title: ts-rust-symmetry
layer: recall
category: cross-layer
keywords: [cross-layer,symmetry,sole_platform,Rust,TypeScript,判定对称]
status: active
---

## 单启用平台判定对称性 (Rust ↔ TS)

## 跨层对称硬规 (Rust ↔ TS)

### 约束

**同一判定逻辑在 Rust 与 TS 各有一份实现，改一处必改另一处。**

口径须与互指注释锁定对称。

### 实现清单

#### 1. Rust 端 (`gateway/router/mod.rs:98-107`)

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

#### 2. TypeScript 端 (`src/domains/groups/GroupIcon.tsx:12-13`)

```typescript
const enabled = gps.filter((gp) => gp.platform.status === "enabled");
const single = gps.length === 1 ? gps[0].platform : enabled.length === 1 ? enabled[0].platform : null;
```

### 对称性检查清单

- [ ] Rust 分支 1（`gps.len() == 1`）↔ TS 分支 1（`gps.length === 1`）
- [ ] Rust 分支 2（`status == Enabled`）↔ TS 分支 2（`status === "enabled"`）
- [ ] 两端都过滤 Enabled 状态（其他 Disabled/AutoDisabled 被排除）
- [ ] 两端都返回 Option/null（无匹配时安全回退）
- [ ] 两端都不考虑 expires_at/disable_during_peak

### 验证

修改任一侧后必须同时修改另一侧：

```bash
# 检查 Rust 实现是否改动
git diff HEAD src-tauri/crates/aidog_core/src/gateway/router/mod.rs | grep -A 10 "sole_platform"

# 检查 TS 实现是否同步改动
git diff HEAD src/domains/groups/GroupIcon.tsx | grep -A 2 "enabled.length === 1"
```

### 注释互指

- **Rust** 文件顶部注释参考 TS 位置
- **TS** 文件（GroupIcon.tsx:7-8）注释参考 Rust 位置和 design.md 位置

### 适用场景

- 判定口径变更（如新增临时闸门维度，需同时更新两侧）
- 状态枚举值改名（如 `Enabled` → `Active`）
- 前端徽标/展示逻辑需与后端路由判定保持一致

---
title: bundled-models-fallback
name: bundled-models-fallback
description: 编译期 include_str! + OnceLock 懒加载配置，DB 恒优先的只读兜底策略
type: recall
category: domain
keywords: bundled, include_str, OnceLock, 兜底, 冷启动
---
---
## 触发场景

只读配置数据（models.json 价格表、platform-presets.json）需在 DB 为空或未同步时兜底，
且无需 generated_at 版本仲裁（DB 恒优先）。

## 陷阱 ❌ vs 正解 ✅

**陷阱1**：启动时 seed DB
- ❌ `fn seed_models()` 启动期间 INSERT bundled → DB（IO 阻塞，版本复杂，弃 bundled 当即失效）
- ✅ 启动时不动 DB，读侧兜底：DB 查无结果 → `include_str!` 回退（原 DB 数据继续生效，无覆盖）

**陷阱2**：版本仲裁（generated_at 比对）
- ❌ bundled 版本 > DB 版本 → 覆盖（增加复杂度，违反「DB 恒优先」）
- ✅ 纯读侧兜底，如无需版本控制则 bundled 永不覆盖（仅当 DB 无该行才用）

**陷阱3**：懒加载时重复解析
- ❌ 每次调 `bundled_model_entry("glm-5.2")` 都跑一次 `serde_json::from_str`（O(n) 反序列）
- ✅ `static BUNDLED: OnceLock<serde_json::Value>` 首次解析后缓存（`.get_or_init` 幂等）

## 反例

```rust
// ❌ 启动 seed （版本冲突、IO 阻塞）
#[init]
async fn on_startup() {
    if is_bundled_newer(db).await {
        seed_models(db, include_str!("models.json")).await;
    }
}

// ✅ 读侧兜底（DB 恒优先）
static BUNDLED: OnceLock<serde_json::Value> = OnceLock::new();
async fn resolve_price(db, model_name, ...) {
    let pd = db.get_model_price(model_name).await
        .or_else(|| bundled_model_entry(model_name))  // DB 无 → 兜底
        .ok_or("unknown")?;
    ...
}
fn bundled_model_entry(name: &str) -> Option<&'static Value> {
    BUNDLED.get_or_init(|| {
        serde_json::from_str(include_str!("../../../defaults/models.json"))
            .unwrap_or_default()
    })
    .get("models")?.get(name)
}
```

## 路径计算

`include_str!` 相对路径**从当前 .rs 文件出发**（不是 Cargo.toml 所在目录）：
- `src-tauri/crates/aidog_core/src/gateway/price_sync.rs` 引用
- 目标 `src-tauri/defaults/models.json`
- 相对路径 = `../../../defaults/models.json`（3 级上升）

## 适用

- 只读配置（定价表、平台预设、常量列表）
- 冷启动不依赖 RPC / 版本同步
- DB 可能暂时为空、滞后同步的场景

## 关联

[[time-tiers-apply-idiom]] [[resolve-price-now-ms]]

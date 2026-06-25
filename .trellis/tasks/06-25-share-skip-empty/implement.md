# 实施计划 — share-skip-empty

决策见 `prd.md`「已定决策」。核心: SharePlatform 5 个可选字段加 `skip_serializing_if`，空值从分享串实际剔除。

## S1 — PlatformModels::is_empty 判定方法

`src-tauri/src/gateway/models/platform.rs:48-60` `impl PlatformModels`（已有 `all_values`），加:

```rust
/// 5 槽位全 None 时为空（用于分享串 skip_serializing_if）。
pub fn is_empty(&self) -> bool {
    self.default.is_none()
        && self.sonnet.is_none()
        && self.opus.is_none()
        && self.haiku.is_none()
        && self.gpt.is_none()
}
```

> 复用 `all_values().is_empty()` 也可，但 `all_values` 有 `#[allow(dead_code)]`（暗示无生产调用方），且会分配 Vec；直接字段判定零分配更优。

## S2 — SharePlatform 字段加 skip_serializing_if

`src-tauri/src/commands/platform.rs:107-125` `SharePlatform`，**仅 5 可选字段**加属性（marker + 4 必填不动）:

```rust
#[serde(default, skip_serializing_if = "String::is_empty")]
pub extra: String,
#[serde(default, skip_serializing_if = "PlatformModels::is_empty")]
pub models: PlatformModels,
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub available_models: Vec<String>,
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub endpoints: Vec<PlatformEndpoint>,
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub manual_budgets: Vec<ManualBudget>,
```

> `#[serde(default)]` 保留（反序列化 round-trip 用）。`String::is_empty` / `Vec::is_empty` 是 std 方法可直接作 serde 谓词。`PlatformModels::is_empty` 见 S1。需确认 `PlatformModels` 已 import 进 `commands/platform.rs`（grep 现有引用确认，应已在 use 列表因 SharePlatform 用到）。

**api_key 不动**: 保持 `pub api_key: String` 无 skip（始终保留，即便空串）。

## S3 — 测试

`src-tauri/src/commands/test_platform.rs`（已存在，grep 确认 share 测试位置）加 2 用例:

1. **空配置导出串不含空字段**: 构造 `Platform` 仅必填有值（extra="" / models 全 None / 3 个 Vec 空）→ `platform_share_export` → `serde_yml::to_string` → 断言输出不含 `extra:` / `models:` / `available_models:` / `endpoints:` / `manual_budgets:` key，只含 marker + 4 必填。
2. **round-trip 等价**: 导出串 → `platform_share_parse` → SharePlatform，extra="" / models=default / 3 Vec=[]，与原 Platform 可选字段语义等价。
3. **有值字段保留**: models.sonnet=Some("xxx") → 串含 `models:` 块含 sonnet。

> 若现有测试已覆盖 share 序列化，扩充断言即可；否则新增 `#[test]` 模块。注意 `platform_share_export` 是 async tauri command（需 db State），测试走 serde 层直接 `serde_yml::to_string(&SharePlatform{...})` 验证 skip 行为，绕开 DB。

`cargo test` + `cargo clippy --all-targets -- -D warnings` 全绿。

## S4 — 无前端改动

后端 serde 已剔除空字段，`ShareModal.tsx` 原样渲染 `<pre>{text}</pre>`（text 来自 yamlStringify/json/base64 of share 对象）即自动清爽。无需改前端。

## 执行顺序

单 agent 顺序 S1 → S2 → S3（字段+判定方法+测试，单文件 struct 域，无跨层）。完成后 main 跑 check。

## 验收 (全过才算完)

1. cargo test + clippy 全绿
2. 空配置分享串 YAML 三行（marker+4必填=5行），无空字段
3. 有值字段正常出现
4. round-trip parse 成功语义等价

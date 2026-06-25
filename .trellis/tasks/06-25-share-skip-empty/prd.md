# 平台分享零值空值字段不展示

## 需求 (用户)

平台分享（ShareModal）输出的分享串中，所有 value 为零值 / 空值的字段不要展示。

## 现状

`SharePlatform` (`src-tauri/src/commands/platform.rs:107-125`) = marker + 4 必填 + 5 可选：

| 字段 | 类型 | 空值形态 | 当前是否出现在串里 |
| --- | --- | --- | --- |
| `aidog_platform_share` | u32 | 恒 1（marker） | 是（接收端校验依赖，恒保留） |
| `name` / `platform_type` / `base_url` / `api_key` | 必填 | — | 是（必填，恒保留） |
| `extra` | String | `""` | **是（要剔除）** |
| `models` | PlatformModels | `{}`（5 槽位全 None） | **是（要剔除）** |
| `available_models` | Vec\<String\> | `[]` | **是（要剔除）** |
| `endpoints` | Vec\<PlatformEndpoint\> | `[]` | **是（要剔除）** |
| `manual_budgets` | Vec\<ManualBudget\> | `[]` | **是（要剔除）** |

> `PlatformModels` (`models/platform.rs:35-46`) 5 槽位已各自 `skip_serializing_if = "Option::is_none"`，故空对象序列化为 `{}`。SharePlatform 层目前**无** skip_serializing_if，所以 `{}` / `[]` / `""` 仍以空形态出现在分享串。

## 已定决策 (用户裁定)

1. **过滤层 = 后端 serde 剔除**（非前端 UI 隐藏）：SharePlatform 字段加 `skip_serializing_if`，空值字段**实际从分享串消失**，YAML / JSON / Base64 三格式统一生效，复制给别人的串也不含空字段。
2. **api_key 始终保留**：即便为空也保留（分享核心字段，空 key 便于接收端察觉异常）。
3. **marker + 4 必填恒保留**：`aidog_platform_share` / `name` / `platform_type` / `base_url` / `api_key` 不加 skip。
4. **5 可选字段空值剔除**：`extra` / `models` / `available_models` / `endpoints` / `manual_budgets`。

## round-trip 安全性

`SharePlatform` 5 个可选字段均已有 `#[serde(default)]`（platform.rs:115-124）。skip 后反序列化（`platform_share_parse` serde_yml）缺字段时回填 default（`""` / `PlatformModels::default()`（全 None）/ `vec![]`），还原语义等价，**无回归**。

## 验收

1. 空配置平台导出分享串（YAML/JSON/Base64 三格式）只含 marker + 4 必填，无空 `extra` / `models: {}` / `available_models: []` / `endpoints: []` / `manual_budgets: []`
2. 有值字段正常出现（任一 models 槽位有值 → 整块 `models` 保留）
3. round-trip：导出串 → `platform_share_parse` 反序列化成功，字段语义等价
4. `cargo test` + `cargo clippy --all-targets -- -D warnings` 全绿
5. warning 必须清

## 不改

- `ShareModal.tsx` 前端展示逻辑（后端已剔除，前端原样渲染即清爽）
- `platform_share_export` / `platform_share_parse` 命令签名 / 返回类型
- `PlatformModels` 内部结构（仅在 impl 加 `is_empty` 判定方法）

---
title: mock-config-4layer-consistency
layer: recall
category: ts-rust-boundary
keywords: [ts-rust-boundary,mock-config,consistency,serde,json-boundary]
status: active
---

## mock 配置四层覆盖的字段一致性检查

## 问题

mock 配置在四层跨 Rust↔TS 边界流转，任一处字段定义/序列化不一致都导致静默失配：

1. **Rust struct** (`config.rs:11-25`)：`Option<u64>` vs `u64`
2. **JSON 序列化** (`config.rs:93-132`)：根据 `body_json["mock"]` 逐字段解析，undefined/缺失→None
3. **TS 类型** (`manual.ts:467-485`)：`field?: number` vs `number`
4. **JSON 反序列化** (`platforms.ts:124-134`)：spread merge + undefined 自动丢弃

## 失配场景

| 症状 | 原因 |
|---|---|
| TS 编辑器赋值后无效 | `serializeMockConfig` 漏字段导致 Rust 端 json.get() 失败 |
| Rust 打印日志不是预期值 | TS 端 undefined 被 stringify 丢弃，Rust 端作 None，取 unwrap_or(默认值) |
| 列表页回显错误 | `parseMockConfig` 漏处理导致 spread merge 不完整 |
| 跨浏览器 localStorage 同步异常 | JSON undefined/null 混用 |

## 检查表（四处同步）

### 1. Rust struct 定义 (`config.rs:11-25`)
- [ ] 新字段声明的类型：`Option<T>` (可选) vs `T` (必须)
- [ ] `#[serde(default)]` 必须加在 struct 级别
- [ ] 默认值对应正确（见 Default impl）

### 2. Rust 解析 (`config.rs:93-132`)
- [ ] body.mock 的每个新字段都有对应的 `if let Some(v) = mock_obj.get("xxx")` 分支
- [ ] 新 Option 字段用 `Some(v)` 包裹

### 3. TS 类型 (`manual.ts:467-485`)
- [ ] 可选字段用 `field?: T`
- [ ] 必须字段用 `field: T`
- [ ] 与 Rust struct 逐字对应

### 4. TS 序列化 (`platforms.ts:124-153`)
- [ ] `parseMockConfig`：spread merge 时包含所有字段
- [ ] `serializeMockConfig`：`obj.mock = mock` 后整体 stringify（undefined 自动丢弃）
- [ ] 编辑器赋值的空串需转 undefined（若有特殊处理）

## 用途

Rust↔TS 跨边界的配置字段迭代通用检查表。适用于：
- 平台/插件配置扩展
- 新增可选设置
- 配置升级 migration

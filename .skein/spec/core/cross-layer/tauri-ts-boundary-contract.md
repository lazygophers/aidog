---
name: tauri-ts-boundary-contract
title: Tauri↔React 边界的三层契约对齐
layer: core
category: cross-layer
keywords: [tauri,rust,typescript,invoke,snake_case,serde]
created: 1725080438
inclusion: auto
---

## 三层契约

1. **Rust struct 字段** → 2. **#[tauri::command] 签名** → 3. **前端 `src/services/api/` invoke 包装**

## 硬约则

- 新增 Tauri command 必须同时补前端 `src/services/api/<domain>.ts` invoke 包装
- invoke 返回值 MUST 标注泛型：`invoke<T>(command, args)`
- Rust 所有字段 MUST `snake_case`；前端 invoke 参数 key 顶级 camelCase，嵌套字段 snake_case
- update payload 含 `#[serde(default)]` 字段时，前端 MUST 传全量值（缺省即 default 覆盖，导致静默清空数据）

## 验证（file:line）

- `src-tauri/src/startup.rs:41+`：generate_handler! 注册表（invoke 名真值源）
- `src/services/api/index.ts:1-21`：40 个 xxxApi barrel export
- `src/services/api/types/generated/`：59 个 ts-rs 生成文件（禁手改）

## 禁用

❌ 仅后端加 command，前端漏 invoke 包装 → 形同死代码  
❌ 字段非 snake_case → serde 反序列化失败，前端拿 undefined  
❌ update 字段漏传 → #[serde(default)] 覆盖，静默清空已存数据

## 关联

[[sole-platform-symmetry]]

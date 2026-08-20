---
layer: recall
created: 1785514813
title: cross-crate-test-path
category: test
keywords: [cross-crate,testing,integration,workspace,test-utils]
status: active
inclusion: auto
---
layer: recall
created: 1785514813

## 跨 Crate 测试路径

## 触发场景
测试代码从外部 crate 迁移进 aidog_core 内部时。

## 陷阱
保持原外部 crate 的全限定路径 `aidog_core::xxx::yyy`，但新位置是 aidog_core 内部，导致编译错误或隐式路径错误。

## 正解
将所有 `aidog_core::` 前缀改为 `crate::`（当前 crate 的自引用）：
```rust
// 迁入前（外部 crate 的 test）
#[test]
fn test_foo() {
  aidog_core::some_module::foo();
}

// 迁入后（aidog_core 内部的 test）
#[test]
fn test_foo() {
  crate::some_module::foo();
}
```

## 案例

## 适用
- 跨 crate 迁移测试文件
- 模块合并时
- 测试代码路径清理

## 关联
[[invoke-name-source-of-truth]]

---
title: 迁入 aidog_core 的测试文件须改 aidog_core:: → crate::
layer: recall
category: test
keywords: [test,migration,module,internal,path]
source: -
authored-by: skein-spec
created: 1785226239
status: active
related: []
updated: 1785226239
---

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
- arch-deepen-2 c3-commands batch 3：迁 commands_*::src/test_*.rs 入 aidog_core

## 适用
- 跨 crate 迁移测试文件
- 模块合并时
- 测试代码路径清理

## 关联
[[Cargo_workspace_重构门禁]] [[command_名集合零差集自检法]]

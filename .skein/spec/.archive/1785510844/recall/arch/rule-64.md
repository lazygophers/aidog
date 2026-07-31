---
title: tauri_command! 宏不支持 mut 形参
layer: recall
category: arch
keywords: [tauri,command,macro,parameter,mut]
source: -
authored-by: skein-spec
created: 1785226231
status: active
related: []
updated: 1785226231
---

## 触发场景
Tauri command 函数形参中使用 `mut` 修饰时。

## 陷阱
`tauri_command!` 宏模式 `$($arg:ident : $ty:ty),*` 不匹配 `mut x: T` 语法，导致编译失败。

## 正解
去掉函数签名中的 `mut`，在函数体首行用 `let mut x = x;` 重绑定：
```rust
// 错误
#[tauri::command]
fn my_cmd(mut state: State) -> Result<()> { ... }

// 正确
#[tauri::command]
fn my_cmd(state: State) -> Result<()> {
  let mut state = state;  // 重绑定
  ...
}
```

## 案例
- arch-deepen-2：迁 command 时遇此限制

## 适用
- Tauri command 签名设计
- 其他 proc macro 类似限制排查

## 关联

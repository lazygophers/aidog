---
title: tauri-command-log-plain-vs-macro
layer: recall
category: arch
keywords: [logging,tauri,macro,instrumentation,command]
source: command_macro.rs + commit 22e18046
authored-by: skein-spec
created: 1722470400
status: active
related: []
updated: 1722470400
---

## 触发场景
删除 `#[tauri::command]` 函数内的手写 `tracing::debug!("command invoked")` 日志时。

## 陷阱
`tauri_command!` 宏自动生成 debug 日志，但 **plain `#[tauri::command]`**（未被宏包裹）的函数**无宏日志**。混淆两者会导致：
- 误删 plain 函数的**唯一日志源** → 运行时无观测、问题难排查
- 只看宏定义无法判断函数是否被宏包裹

## 正解
删手写 debug! 前必须先确认该函数**真的被 `tauri_command!` 宏包裹**：

```rust
// ✅ 被宏包裹 → 删宏自动生成的 debug! 是安全的
tauri_command! {
  pub async fn foo() -> Result<String, String> {
    tracing::debug!("command invoked");  // ← 可删（宏已发）
    ...
  }
}

// ❌ plain #[tauri::command] → 这行是**唯一日志**，不能删
#[tauri::command]
pub async fn bar() -> Result<String, String> {
  tracing::debug!("command invoked");  // ← 绝对不能删
  ...
}
```

## 检查清单
- grep 该函数名，确认在 startup.rs 的 generate_handler! 中
- 在 command 实现处，往上搜 `tauri_command!` 宏开头或 `#[tauri::command]` 属性
- 若无宏声明只有属性，该函数是 plain，手写 debug! 不可删

## 案例
提交 22e18046（chore: 删与 tauri_command! 宏重复的日志 87 处）：
- 删了 87 处在 `tauri_command!` 宏内的重复 debug!
- 保留了 113 处：100 处携带宏 skip_all 捕不到的额外字段，**13 处是 plain 函数的唯一日志**

## 适用
- 新增 command 后续删冗余日志
- 代码审查监控日志重复

## 不违反此规则会导致
- plain 函数无日志 → 请求链路不可观测
- 无法判断 command 是否被调用
- 性能问题排查时缺关键上下文

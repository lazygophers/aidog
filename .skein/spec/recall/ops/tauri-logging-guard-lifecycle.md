---
layer: recall
created: 1785514813
title: tauri-logging-guard-lifecycle
category: ops
keywords: [Tauri,tracing,WorkerGuard,logging,lifecycle,guard-management]
status: active
inclusion: auto
---
layer: recall
created: 1785514813

## Tauri tracing_appender::non_blocking WorkerGuard 生命周期陷阱

## Tauri `tracing_appender::non_blocking` WorkerGuard 生命周期陷阱

### 触发场景

在 Tauri 应用中使用 `tracing_appender::non_blocking` 创建后台日志写线程，未正确管理 `WorkerGuard` 生命周期。

### 缺陷：guard drop 即停后台写线程，缓冲日志全丢

`tracing_appender::non_blocking` 返回 `(NonBlockingWriter, WorkerGuard)` 元组。**WorkerGuard 包含运行中的后台写线程引用**；guard 被 drop 时，后台线程立即停止，缓冲中未刷新的日志**静默丢失**。

常见误用：
```rust
let (writer, guard) = non_blocking(file_appender);  // guard 是局部变量！
let subscriber = fmt()
  .with_writer(writer)
  .finish();

// guard 在作用域末尾自动 drop，后台线程停止
// 后续所有日志丢失（写到已关闭的 channel）
```

### 正解：bind guard to app lifecycle

WorkerGuard 的生命周期必须与应用同长，即从应用启动到关闭。Tauri 应用的状态表（via `app.manage()`）正好管理这个生命周期。

**代码证据** (`src-tauri/src/app_setup.rs:106-110`)：
```rust
// WorkerGuard 生命周期契约: non-blocking 文件写后台线程随 guard drop 而停。
// `app.manage` 存进 Tauri 状态表, 与 App/AppHandle 同生共死, 覆盖到进程退出前
// 最后一刻 —— 不绑局部变量 (那样 setup() 一返回 guard 就没了, 见 init_logging 文档)。
if let Some(guard) = logging::init_logging(&data_dir, &log_settings) {
    app.manage(guard);
}
```

guard 在应用关闭时 drop，后台线程安全停止 + 刷新缓冲。

### 不选别的理由

| 备选 | 否决 |
|---|---|
| 把 guard 存全局 static | 全局泄露 guard，线程不会停止，app 无法清理日志 fd |
| 用 Arc<Mutex<Guard>> 手工延长生命周期 | Tauri 状态表已做了这件事，重复手工管理会破坏一致性 |
| 忽视 guard，靠 async 写者自行缓冲 | 那不是 `non_blocking` 的设计意图；writer 本质是 async channel，drop 即关闭 |

### 验收

- [ ] grep `non_blocking` 全 source，找到所有创建点
- [ ] 每处 guard 都已 `app.manage()` 或绑定应用状态
- [ ] 无 guard 作为局部变量在函数末尾被 drop
- [ ] 有用例验证「应用关闭前所有日志已刷新到文件」

### 适用

- Tauri 桌面应用所有使用 `tracing_appender::non_blocking` 的代码
- 通用：任何后台线程处理缓冲数据时，guard 的生命周期必须显式绑定到应用生命周期

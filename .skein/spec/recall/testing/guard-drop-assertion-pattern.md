---
title: guard-drop-assertion-pattern
layer: recall
category: testing
keywords: [testing,resources,lifecycle,guard,tracing]
source: src-tauri/crates/aidog_core/src/logging.rs:561-592
authored-by: skein-spec
created: 1722470400
status: active
related: []
updated: 1722470400
---

## 触发场景
异步化迁移中，从 sync 改为 async 操作，需验证资源保活和清理行为时。

## 陷阱
空谈「应该不会丢」，无法实际验证。常见问题：
- WorkerGuard drop 时后台线程没及时 flush channel
- 局部变量作用域内 drop guard，后续业务在无 subscriber 状态下运行
- 隐式依赖全局状态，迁移后状态初始化漏了某个环节

## 正解
写一个自包含的测试，模拟「退出」场景，验证清理行为：

```rust
#[test]
fn worker_guard_drop_flushes_pending_writes() {
    use std::io::Write as _;
    let tmp = tempfile::tempdir().expect("tempdir");
    let settings = AppLogSettings { ... };
    let (mut non_blocking, guard, log_dir) = 
        build_file_appender(tmp.path(), &settings).expect("appender");
    
    // 1. 写入数据（模拟正常运行）
    const N: usize = 500;
    for i in 0..N {
        writeln!(non_blocking, "line {i}").expect("write");
    }
    
    // 2. 主动 drop guard（模拟进程退出）
    drop(guard);
    
    // 3. 断言数据全部落盘（不是丢弃）
    let mut lines_total = 0usize;
    for entry in std::fs::read_dir(&log_dir).expect("read") {
        let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
        lines_total += content.lines().filter(|l| l.starts_with("line ")).count();
    }
    assert_eq!(lines_total, N, "all {N} lines must flush after drop");
}
```

## 检查清单
- [ ] 测试创建隔离环境（tempdir / 独立 test db 等）
- [ ] 异步行为前后都有明确行为（写 data → drop → 读磁盘）
- [ ] 断言是**可观测**的（磁盘文件、数据库行数、成功/失败 enum）
- [ ] 若 drop 过早（bug 重现），测试会因行数不足而失败
- [ ] 无 `Thread::sleep` 轮询，用原始同步操作（drop → 读）验证

## 案例
`logging.rs` 的 `worker_guard_drop_flushes_pending_writes` (line 567-592)：
- 步骤 1：创建 file appender + 写 500 行
- 步骤 2：立即 drop guard（不等后台消化）
- 步骤 3：读日志目录，断言全 500 行都在磁盘上

若 bug（guard 提前 drop / channel 未 flush），test 失败。

## 适用
- 任何 drop-on-exit 的资源（guard、subscription、lock）
- 后台线程与主线程数据交接（channel flush）
- 缓存/buffer 异步持久化

## 不适用
- 纯同步操作（已通过编译器验证生命周期）
- 业务逻辑单测（应独立测，不涉及资源保活）

## 副作用
- 测试代码本身复杂（需模拟退出 + 磁盘 I/O）
- 需要 tempdir / 临时资源创建，测试时间略长

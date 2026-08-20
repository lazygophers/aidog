---
title: command 名集合零差集自检法（搬迁类）
layer: recall
category: arch
keywords: [command,tauri,handler,migration,invoke,symmetry]
source: -
authored-by: skein-spec
created: 1785226199
status: active
related: []
updated: 1785226199
---

## 触发场景
command 跨 crate 搬迁后（新增、删除、拆分 command）。

## 陷阱
改了 Rust 函数签名或迁移位置，却漏改了前端 invoke 名或 startup.rs 注册，导致静默失败。

## 正解
**invoke 名的真值源 = `src-tauri/src/startup.rs:41` 的 `tauri::generate_handler!` 集合**（由 `#[tauri::command]` 函数名自动收集，与模块路径无关）。

搬迁前后自检：
```bash
# 抽取两份集合
grep "#\\[tauri::command\\]" -A1 src-tauri/src/**/*.rs | grep "fn " | awk '{print $2}' | sort > before.txt
grep "#\\[tauri::command\\]" -A1 src-tauri/src/**/*.rs | grep "fn " | awk '{print $2}' | sort > after.txt

# 零差集验证
comm -3 before.txt after.txt  # 应为空
```

## 案例
- arch-deepen-2 batch 3：commands 迁 aidog_core 时，verify 用 comm -3 零差集确认 invoke 名未变

## 适用
- command 跨 crate 搬迁
- 新增/删除 command
- 重构后 sanity check

## 关联

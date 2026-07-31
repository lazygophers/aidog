---
title: rule-61
layer: recall
category: build
keywords: [cargo,clippy,cache,warning,touch,rebuild]
source: -
authored-by: skein-spec
created: 1785226206
status: active
related: []
updated: 1785226206
---
---
## 触发场景
修改后再跑 `cargo clippy` 判断 warning 数时。

## 陷阱
同命令第二次跑输出为空（命中编译缓存），易误判「0 warning」实际仍有。

## 正解
修改源文件后跑 clippy 前，先 `touch` 该文件强制重编：
```bash
touch src-tauri/crates/aidog_core/src/lib.rs
cargo clippy --workspace 2>&1 | grep warning | wc -l
```

## 案例
- arch-deepen-2：迁移函数后 clippy 无新输出，touch 才触发重编检查

## 适用
- 验证 clippy 改动效果
- 高频编译场景
- 持续集成前检查

## 关联
[[rule-63]]

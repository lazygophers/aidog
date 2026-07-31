---
title: rule-63
layer: recall
category: build
keywords: [env,compile-time,build.rs,cargo:rustc-env,scope]
source: -
authored-by: skein-spec
created: 1785226225
status: active
related: []
updated: 1785226225
---
---
## 触发场景
用 `env!("XXX")` 的代码从一个 crate 迁移到另一个 crate 时。

## 陷阱
`cargo:rustc-env=` 在 build.rs 中定义的环境变量**只对定义它的 crate 生效**，跨 crate 后会编译失败或返回空值。

## 正解
迁移代码到新 crate 后，给**新 crate 补等价的 build.rs**，重新定义环境变量。

## 案例
- arch-deepen-2 c3-commands batch 3：commands_tray/commands_system/等迁 aidog_core 时
- 原 commands_tray/build.rs 定义的 `TAURI_ENV_*` 需在 aidog_core/build.rs 重新定义
- crates/aidog_core/build.rs 已补

## 检查
```bash
# 检查迁移后是否仍能编译通过
cargo build -p aidog_core  # 应无 env! 相关错误
```

## 适用
- 任何用 env!() 的代码跨 crate 迁移
- workspace 多 crate 场景
- build.rs 依赖的外部环境变量

## 关联


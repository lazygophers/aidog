---
title: env-broadcast-race-forbidden
layer: core
category: arch
keywords: [concurrency,env,race,onetask,process]
source: gateway/skills/env.rs
authored-by: skein-spec
created: 1722470400
status: active
related: []
updated: 1722470400
---

## 触发场景
探测到一个值（如登录 shell 的 PATH）后，想让所有子进程用上时。

## 陷阱
禁用 `unsafe std::env::set_var` 全局广播。若该探测是惰性触发，触发时刻代理线程/tokio worker 已启动，`set_var` 与其他线程 `getenv` 构成 **data race**（LLVM 会优化掉不确定行为的代码）。

## 正解
1. **缓存合并结果**：用 `OnceLock<Option<String>>` 存探测值（一会话仅触发一次）
2. **各 spawn 显式注入**：在每个 spawn 站点的 `Command` 上 `.env(KEY, value)` 注入
3. **不改全局 env**：禁 `std::env::set_var` 广播

## 案例
`gateway/skills/env.rs:13-28` 的 `runtime_path()` 实现：
- 行 11：`static PATH_CACHE: OnceLock<Option<String>>` 缓存
- 行 26：`PATH_CACHE.get_or_init(probe_login_path)` 幂等探测
- 行 77-95：各 spawn 站点（node_cmd/npx_cmd）调 `.env("PATH", p)` 注入

## 适用
- skills 环境探测（node/npx/python 可用性）
- CLI proxy 子进程启动
- script_executor 脚本运行环境

## 不违反此规则会导致
- 启动窗口 GUI env 极简，代理线程已跑，set_var 无效或竞态
- 下游子进程找不到 brew/nvm/pyenv 装的工具，报「未安装」（实为 PATH 缺失）

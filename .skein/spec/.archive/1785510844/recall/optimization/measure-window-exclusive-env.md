---
title: 环境互斥约束：profiling 与编译竞争
layer: recall
category: optimization
keywords: [profiling,performance,measurement,environment,cargo,yarn,exclusive]
status: active
inclusion: auto
protected: true
---

## 环境互斥约束

Profiling（采样、trace 录制）与后台编译（cargo/yarn build）占用机器资源竞争。同步触发导致采样结果被编译线程偏歪（高于实际空闲态）。

### 硬约束

- profiling 窗口内禁止 `cargo build` / `yarn build` 等后台编译任务
- 若需多轮采样，间隔应 ≥5min 让机器回到空闲（L3 缓存冷却）
- 采样前 `killall node` 等开发服务，确保仅有 app 进程在运行

### 验证方式

- `ps aux | grep -E "cargo|yarn"` 确认无编译进程
- `top -l 1` 瞬时快照应 <1% user CPU（无突发）
- 时间戳对齐：profiling 启动 / 采样完成都精确记录，与后续 commit/build 时间对不上即通过

### 关联

[[idle-cpu-baseline-xctrace]] 取样于此约束下 / [[webkit-jit-warmup-trap]] 采样时机亦受制

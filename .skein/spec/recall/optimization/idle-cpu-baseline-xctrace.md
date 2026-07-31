---
title: 空闲 CPU 基线数据（xctrace 实测）
layer: recall
category: optimization
keywords: [baseline,measurement,xctrace,process,webkit,profiling,cpu]
status: active
inclusion: auto
---

## 空闲 CPU 基线数据

基于 xctrace Time Profiler 实测（2026-07-31，30s 采样窗口）。四进程占比：
- **main(Rust)**: 11ms ≈ 0.037%（全部落框架底噪：tao 事件循环 / tokio runtime / objc 运行时，无业务代码采样命中）
- **WebContent(主窗口)**: 2327ms ≈ 7.757%（JSC/WebCore 框架底噪，dev 模式 Vite 未 bundle 导致反复 JIT）
- **GPU**: 0ms ≈ 0.0000%（整窗无采样）
- **WebContent(popover 隐藏)**: 71ms ≈ 0.237%（补录，其中渲染管线仅 4ms ≈ 0.0133%）

### 后续复测参考值

新复测若测得总和 ≥0.5%：大头仍是 WebContent 框架底噪（范围外），非 S2/S3/S4 业务代码点位。不必因总和未达标而返工已验证的 <0.05% 候选。

### 关联

[[measure-window-exclusive-env]] 环境约束 / [[webkit-jit-warmup-trap]] 采样时机陷阱 / [[idle-wakeup-sources-inventory]] 唤醒源验证

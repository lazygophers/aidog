---
title: 空闲 CPU 基线数据（xctrace 实测）
layer: recall
category: optimization
keywords: [baseline,measurement,xctrace,process,webkit,profiling,cpu]
status: active
inclusion: auto
protected: true
---

## 空闲 CPU 基线数据

基于 xctrace Time Profiler 实测（2026-07-31，30s 采样窗口）。四进程占比：
- **main(Rust)**: 11ms ≈ 0.037%（全部落框架底噪：tao 事件循环 / tokio runtime / objc 运行时，无业务代码采样命中）
- **WebContent(主窗口)**: 2327ms ≈ 7.757%（JSC/WebCore 框架底噪，dev 模式 Vite 未 bundle 导致反复 JIT）
- **GPU**: 0ms ≈ 0.0000%（整窗无采样）
- **WebContent(popover 隐藏)**: 71ms ≈ 0.237%（补录，其中渲染管线仅 4ms ≈ 0.0133%）

### 稳态基线（后续 task 用这一组，不是上面那组）

上面是**启动后不久**的 30s Profiler 采样，WebContent 那 7.757% 是 JIT 热身突发。运行 45min 后
`top -l 65 -s 1` 65s 均值才是稳态基线：

- **main(Rust)**: 0.1200%
- **WebContent(主窗口)**: 0.0154%
- **WebContent(popover)**: 0.0000%
- **GPU**: 0.0077%
- **总和**: **0.1431%**（远低于 0.5% 阈值）

后续性能 task 拿 0.1431% 当基线，别再从零重测。引 30s 那组数据谈稳态开销会高估两个量级 ——
见 [[webkit-jit-warmup-trap]]。

### 关联

[[measure-window-exclusive-env]] 环境约束 / [[webkit-jit-warmup-trap]] 采样时机陷阱 / [[idle-wakeup-sources-inventory]] 唤醒源验证

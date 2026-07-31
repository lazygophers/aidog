---
title: WebContent JSC JIT 热身陷阱：采样时机决定结论
layer: recall
category: optimization
keywords: [webkit,jsc,jit,warmup,profiling,sampling,trap,cpu]
status: active
inclusion: auto
---

## WebContent JSC JIT 热身陷阱

WebContent 进程中 JSC JIT 热身阶段（启动后数分钟）vs 稳定态（运行 45+ 分钟）的 CPU 占比差异巨大：启动 7.757% vs 运行 45min 后 0.0154%（差两个量级）。

### 陷阱

未等热身结束即采样导出的结论无法代表稳态空闲 CPU。例：
- 启动后 3 分钟内 profiling 会错误推断「WebContent 是主要开销」
- 需 wait-for-idle 等待 45 分钟后才能判定长期占比

### 采样策略

1. 启动 app → 等待 Web 加载完成（窗口响应正常）
2. **等待 30 分钟以上** 让 JSC JIT 编译完成 / GC 稳定
3. 然后再跑 30s xctrace 采样
4. 结果才能信任作「空闲 CPU 基线」

### 原因

dev 模式下 Vite 提供未 bundle 的 JS 资源，JSC 需完整 Lexer → DFG(Data Flow Graph) JIT → FTL(Faster Than Light) JIT 三阶升温，伴随反复 GC。release build 预 bundle + minify 后体积和解析次数大幅降低，理论上此项显著更低。

### 关联

[[idle-cpu-baseline-xctrace]] 本数据依此陷阱修正而来

---
title: 栈归因用法：从静态检索到 profiling 验证
layer: recall
category: ops
keywords: [profiling,stack-trace,attribution,instruments,xctrace,methodology,cpu]
status: active
inclusion: auto
---

## 栈归因用法

**定理**：静态检索定时器只能估出量级（因周期、触发条件、执行成本都是猜），无法判断是否真在稳态 CPU 占比中命中。必须栈归因验证。

### 正确用法

1. **静态阶段**：`rg` 定位候选点位（scheduler / tray / 定时任务），**推测** CPU 占比上限（"极低频 60s → 均摊 0.01% 量级"）
2. **profiling 阶段**：用 xctrace Time Profiler 采 30s，导出 `time-profile` schema（非 `time-sample`），检查该候选点位是否有栈帧命中
3. **判定**：
   - 有采样点 → 观察帧名、调用链、总权重 → 推断当前真实占比
   - 无采样点 → 证实该候选上限 < (总采样间隔 / 采样密度)，符合 0.01% 推测，清零
   - 栈名不匹配 → 需查源码验证符号名（如 `gateway/backup/scheduler` 对应何帧名）

### 工具链

- `xcrun xctrace record --template 'Time Profiler' --attach <pid> --time-limit 30s`（Xcode CLT 自带）
- 结果 Instruments.app 中右上 "Inspect" 导出 xml / 或直接读 .trace 二进制（`instruments -l thick <trace.trace>`）
- 无需 GUI：CLI `xctrace` 可直接生成可读文本报表

### 反面案例

- "backup scheduler 60s 轮询应该消耗 0.01%，所以不必改" → 错。需栈归因验证确实无采样点，才能确定不改
- "WebContent 开销是 JSC JIT，我看不到本项目代码，所以不用优化" → 偷懒。栈证明后才能做此结论，否则可能有 RC 漏洞误触发了 XCTests

### 关联

[[measure-window-exclusive-env]] 保证采样环境清净 / [[idle-cpu-baseline-xctrace]] 本报告取样结果 / [[webkit-jit-warmup-trap]] 采样前必 wait

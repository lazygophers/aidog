---
title: idle-cpu-stack-sampling
layer: recall
category: optimization
keywords: [cpu,profiling,sample,timer,instruments,time-profiler]
status: active
protected: true
---

## 空闲 CPU 归因必须靠栈采样

## 触发场景

性能分析中发现应用稳态 CPU 占用 3.0%，但静态代码检索只能找到 60s×1 + 300s×1 + 24h×3 共 5 个定时器。这 5 个定时器的工作量加总无法解释 3.0% 稳态 CPU，缺口需要用栈采样才能诊断。

## 陷阱 & 正解

❌ **陷阱**：仅用静态代码检索（grep）列举定时器

```bash
# 搜索所有定时器
grep -r "setTimeout\|setInterval\|requestAnimationFrame" src/

# 结果：60s + 300s + 24h×3 定时器
# 加总工作量 = Y units
# 但实测 CPU = 3.0%，无法对齐
```

代码检索完整但不能说明执行时的真实工作分布（某定时器条件执行、某回调被优化、GC 压力等）。

✅ **正解**：用 Instruments Time Profiler 或 `sample` 命令做栈采样，捕捉真实热点

```bash
# 启动应用
open /Applications/aidog.app

# 采样 30 秒
sample <pid> 30 > cpu-profile.txt

# 或用 Instruments（图形化）
instruments -t "Time Profiler" /Applications/aidog.app &

# 分析输出，找出占用最多 CPU 的调用栈
# 可能发现：
# - 某个 event listener 循环调用
# - 某定时器内部 while 循环
# - DOM 频繁重排的回调链
```

栈采样直接展示 CPU 时间的真实去向，定位热点函数。

## 反例（错误模式）

| ❌ 错 | ✅ 改为 |
|---|---|
| 仅 grep 定时器列表 | grep 列表 + `sample` 栈采样验证实际 CPU |
| 假设所有定时器工作量相等 | 栈采样显示各函数实际执行时间占比 |
| 缺口无法解释 → 放弃 | 缺口 → 启用 Time Profiler 深挖隐式循环/GC/DOM |

## 案例

grep 找到 5 个定时器，工作量推算应占 CPU 1-1.5%。但实测 3.0% 稳态，缺口 1.5% 无法追溯。用 `sample` 采样后发现某 event listener 在定时器回调中被多次调用（原代码遗漏），DOM 重排放大了工作量。改为事件去重后，定时器 + event 总工作量回归对齐。

## 适用

- 稳态 CPU 3% 以上但代码检索无法解释的场景
- 长时间后台进程 CPU 诊断
- 定时任务链效应分析（A 定时器→B 回调→C 事件→D GC）

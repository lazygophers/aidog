# SKEIN recall 关联表 (A-MEM-lite 正反链)

章节粒度: 规则 id = `<类目>/<主题>.md#<规则标题>`; `←` 入链 (谁引用本条) / `→` 出链 (本条引用谁)。无条目 = 孤立候选。

## db/sqlite-cache-residency-probe-method.md#SQLite 页缓存常驻量的直接探针方法
- ← core/db/sqlite-read-cache-config.md#关联

## db/sqlite-cache-residency-probe-method.md#页缓存常驻量探针
- ← core/db/sqlite-read-cache-config.md#关联
- → [[measure-window-exclusive-env]]
- → [[sqlite-cache-measurement-traps]]
- → [[sqlite-read-cache-config]]

## domain/prd-acceptance-consistency-check.md#PRD 验收标准与约束互容性检查
- → [[mock-platform-bypasses-forward-pipeline]]

## ops/idle-wakeup-sources-inventory.md#空闲期唤醒源 6 分类清单
- ← recall/optimization/idle-cpu-baseline-xctrace.md#空闲 CPU 基线数据
- → [[idle-cpu-baseline-xctrace]]
- → [[measure-window-exclusive-env]]

## ops/logging-queue-capacity-tuning.md#日志队列 capacity 定值方法：从采样均值反推
- → [[hot-path-buffers]]

## ops/stack-attribution-profiling-methodology.md#栈归因用法
- → [[idle-cpu-baseline-xctrace]]
- → [[measure-window-exclusive-env]]
- → [[webkit-jit-warmup-trap]]

## ops/test-data-isolation-constraint.md#量测脚本 HOME 环境隔离硬约束
- → [["$HOME" == "$HOME_REAL"]]
- → [[tmp]]

## optimization/idle-cpu-baseline-xctrace.md#空闲 CPU 基线数据
- ← recall/ops/idle-wakeup-sources-inventory.md#空闲期唤醒源 6 分类清单
- ← recall/ops/stack-attribution-profiling-methodology.md#栈归因用法
- ← recall/optimization/measure-window-exclusive-env.md#环境互斥约束
- ← recall/optimization/webkit-jit-warmup-trap.md#WebContent JSC JIT 热身陷阱
- → [[idle-wakeup-sources-inventory]]
- → [[measure-window-exclusive-env]]
- → [[webkit-jit-warmup-trap]]

## optimization/measure-window-exclusive-env.md#环境互斥约束
- ← recall/db/sqlite-cache-residency-probe-method.md#页缓存常驻量探针
- ← recall/ops/idle-wakeup-sources-inventory.md#空闲期唤醒源 6 分类清单
- ← recall/ops/stack-attribution-profiling-methodology.md#栈归因用法
- ← recall/optimization/idle-cpu-baseline-xctrace.md#空闲 CPU 基线数据
- → [[idle-cpu-baseline-xctrace]]
- → [[webkit-jit-warmup-trap]]

## optimization/sqlite-cache-measurement-traps.md#SQLite 页缓存量测陷阱
- ← recall/db/sqlite-cache-residency-probe-method.md#页缓存常驻量探针

## optimization/sqlite-cache-measurement-traps.md#SQLite 页缓存量测三大陷阱
- ← recall/db/sqlite-cache-residency-probe-method.md#页缓存常驻量探针

## optimization/webkit-jit-warmup-trap.md#WebContent JSC JIT 热身陷阱
- ← recall/ops/stack-attribution-profiling-methodology.md#栈归因用法
- ← recall/optimization/idle-cpu-baseline-xctrace.md#空闲 CPU 基线数据
- ← recall/optimization/measure-window-exclusive-env.md#环境互斥约束
- → [[idle-cpu-baseline-xctrace]]

## skein/parallel-subtask-prop-contract.md#关联
- → [[dirty-float-hour-normalization]]
- → [[form-level-tz-state-sharing]]

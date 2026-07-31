# SKEIN recall 关联表 (A-MEM-lite 正反链)

章节粒度: 规则 id = `<类目>/<主题>.md#<规则标题>`; `←` 入链 (谁引用本条) / `→` 出链 (本条引用谁)。无条目 = 孤立候选。

## arch/adapter-deadcode-whitelist-authority.md#关联
- → [[rule-07]]

## arch/agent-platform-handler-branch.md#关联
- → [[trellis-04]]

## arch/component-extraction-grep-callsites.md#关联
- → [[auto-fix-downgrade-36]]

## arch/cross-db-subquery-handle-selection.md#MUST 规则
- ← recall/arch/db-split-access-point-audit.md#关联

## arch/cross-db-subquery-handle-selection.md#错误样本（❌）
- ← recall/arch/db-split-access-point-audit.md#关联

## arch/cross-db-subquery-handle-selection.md#正确写法（✅）
- ← recall/arch/db-split-access-point-audit.md#关联

## arch/cross-db-subquery-handle-selection.md#验收
- ← recall/arch/db-split-access-point-audit.md#关联

## arch/cross-db-subquery-handle-selection.md#Cross-ref
- ← recall/arch/db-split-access-point-audit.md#关联
- → [[auto-fix-downgrade-34]]

## arch/db-split-access-point-audit.md#关联
- → [[auto-fix-downgrade-35]]
- → [[cross-db-subquery-handle-selection]]

## arch/dedup-key-must-be-nonempty.md#关联
- → [[shadcn-infra-32]]

## arch/enum-variant-delete-needs-migration.md#关联
- → [[shadcn-infra-32]]
- → [[trellis-04]]

## arch/gemini-sse-alt-param.md#关联
- → [[rule-57]]
- → [[rule-58]]

## arch/locale-deadkey-cleanup-ownership.md#关联
- → [[auto-fix-downgrade-38]]

## arch/protocol-wire-str.md#关联
- → [[rule-05]]

## arch/tauri-popover-window-reuse.md#关联
- → [[rule-45]]
- → [[trellis-03]]
- → [[trellis-18]]

## build/build-rs-env-is-crate-scoped.md#关联
- → [[rule-61]]

## build/clippy-touch-before-recheck.md#关联
- → [[rule-63]]

## build/converter-endpoint-decoupled.md#案例
- → [[rule-07]]
- → [[rule-55]]

## build/shadcn-add-verify-deps.md#关联
- → [[shadcn-infra-31]]

## build/tailwind-v4-import-form.md#关联
- → [[shadcn-infra-28]]
- → [[shadcn-infra-30]]

## build/vite-at-alias-manual.md#关联
- → [[shadcn-infra-28]]

## build/wire-protocol-gate-is-failfast.md#案例
- → [[rule-05]]
- → [[rule-54]]

## db/sqlite-cache-residency-probe-method.md#SQLite 页缓存常驻量的直接探针方法
- ← core/db/sqlite-read-cache-config.md#关联

## db/sqlite-cache-residency-probe-method.md#页缓存常驻量探针
- ← core/db/sqlite-read-cache-config.md#关联
- → [[measure-window-exclusive-env]]
- → [[sqlite-cache-measurement-traps]]
- → [[sqlite-read-cache-config]]

## domain/bundled-models-fallback.md#触发场景
- ← recall/domain/time-tiers-apply-idiom.md#关联

## domain/bundled-models-fallback.md#陷阱 ❌ vs 正解 ✅
- ← recall/domain/time-tiers-apply-idiom.md#关联

## domain/bundled-models-fallback.md#反例
- ← recall/domain/time-tiers-apply-idiom.md#关联

## domain/bundled-models-fallback.md#路径计算
- ← recall/domain/time-tiers-apply-idiom.md#关联

## domain/bundled-models-fallback.md#适用
- ← recall/domain/time-tiers-apply-idiom.md#关联

## domain/bundled-models-fallback.md#关联
- ← recall/domain/time-tiers-apply-idiom.md#关联
- → [[rule-66]]
- → [[time-tiers-apply-idiom]]

## domain/converter-normalized-intermediate.md#关联
- → [[rule-52]]
- → [[rule-54]]

## domain/cpa-oauth-credential-format.md#多账号语义（CLIProxyAPI）
- → [[auto-fix-downgrade-35]]

## domain/cpa-oauth-credential-format.md#Cross-ref
- → [[auto-fix-downgrade-35]]
- → [[parser-multi-path-format-symmetry]]

## domain/endpoint-cross-protocol-fallback.md#案例
- → [[rule-06]]
- → [[rule-07]]

## domain/five-wire-protocols-anchor.md#关联
- → [[rule-05]]
- → [[rule-53]]

## domain/prd-acceptance-consistency-check.md#PRD 验收标准与约束互容性检查
- → [[mock-platform-bypasses-forward-pipeline]]

## domain/reasoning-content-as-text-block.md#关联
- → [[rule-52]]
- → [[rule-53]]

## domain/time-tiers-apply-idiom.md#触发场景
- ← core/domain/peak-multiplier-symmetry.md#关联
- ← recall/domain/bundled-models-fallback.md#关联

## domain/time-tiers-apply-idiom.md#陷阱 ❌ vs 正解 ✅
- ← core/domain/peak-multiplier-symmetry.md#关联
- ← recall/domain/bundled-models-fallback.md#关联

## domain/time-tiers-apply-idiom.md#反例
- ← core/domain/peak-multiplier-symmetry.md#关联
- ← recall/domain/bundled-models-fallback.md#关联

## domain/time-tiers-apply-idiom.md#案例
- ← core/domain/peak-multiplier-symmetry.md#关联
- ← recall/domain/bundled-models-fallback.md#关联

## domain/time-tiers-apply-idiom.md#适用
- ← core/domain/peak-multiplier-symmetry.md#关联
- ← recall/domain/bundled-models-fallback.md#关联

## domain/time-tiers-apply-idiom.md#关联
- ← core/domain/peak-multiplier-symmetry.md#关联
- ← recall/domain/bundled-models-fallback.md#关联
- → [[bundled-models-fallback]]
- → [[rule-66]]
- → [[rule-67]]

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

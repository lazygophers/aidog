# SKEIN core 关联表 (A-MEM-lite 正反链)

章节粒度: 规则 id = `<类目>/<主题>.md#<规则标题>`; `←` 入链 (谁引用本条) / `→` 出链 (本条引用谁)。无条目 = 孤立候选。

## arch/mock-platform-short-circuit.md#Mock 平台绕开转发流水线短路
- ← core/proxy/auto-disable-401-403-402.md#关联

## arch/stream-buf-unified-cap.md#硬约则
- ← core/perf/stream-buf-no-batching.md#关联

## arch/stream-buf-unified-cap.md#案例
- ← core/perf/stream-buf-no-batching.md#关联

## arch/stream-buf-unified-cap.md#适用
- ← core/perf/stream-buf-no-batching.md#关联

## arch/stream-buf-unified-cap.md#关联
- ← core/perf/stream-buf-no-batching.md#关联
- → [[hot-path-buffers]]
- → [[stream-buf-no-batching]]

## cross-layer/tauri-ts-boundary-contract.md#关联
- → [[sole-platform-symmetry]]

## db/connectionclosed-retry.md#关联
- → [[crash-safe-db-split]]
- → [[sqlite-read-cache-config]]

## db/crash-safe-db-split.md#拆库迁移四阶段 Crash-Safe 范式
- ← core/db/connectionclosed-retry.md#关联

## db/sqlite-read-cache-config.md#SQLite 只读缓存定值
- ← core/db/connectionclosed-retry.md#关联
- ← recall/db/sqlite-cache-residency-probe-method.md#页缓存常驻量探针

## db/sqlite-read-cache-config.md#关联
- ← core/db/connectionclosed-retry.md#关联
- ← recall/db/sqlite-cache-residency-probe-method.md#页缓存常驻量探针
- → [[sqlite-cache-residency-probe-method]]

## domain/peak-multiplier-symmetry.md#关联
- → [[rule-66]]
- → [[time-tiers-apply-idiom]]

## i18n/i18n-key-sync-8lang.md#关联
- → [[zh-hans-literal-sync]]

## perf/stream-buf-no-batching.md#关联
- ← core/arch/stream-buf-unified-cap.md#关联
- → [[stream-buf-unified-cap]]

## proxy/auto-disable-401-403-402.md#关联
- → [[http-client-no-env-proxy]]
- → [[mock-platform-short-circuit]]

## proxy/crash-safe-db-split.md#crash safe db split
- ← core/db/connectionclosed-retry.md#关联

## proxy/mock-platform-short-circuit.md#mock platform short circuit
- ← core/proxy/auto-disable-401-403-402.md#关联

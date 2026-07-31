# SKEIN core 关联表 (A-MEM-lite 正反链)

章节粒度: 规则 id = `<类目>/<主题>.md#<规则标题>`; `←` 入链 (谁引用本条) / `→` 出链 (本条引用谁)。无条目 = 孤立候选。

## arch/mock-platform-bypasses-forward-pipeline.md#mock 平台绕开真实转发流水线，无法验证 finish.rs 挂载的 cap/累积逻辑
- ← recall/domain/prd-acceptance-consistency-check.md#PRD 验收标准与约束互容性检查

## arch/mock-platform-bypasses-forward-pipeline.md#硬约束
- ← recall/domain/prd-acceptance-consistency-check.md#PRD 验收标准与约束互容性检查

## arch/mock-platform-bypasses-forward-pipeline.md#关联
- ← recall/domain/prd-acceptance-consistency-check.md#PRD 验收标准与约束互容性检查

## arch/protocol-wire-str.md#触发场景
- ← core/arch/protocol-wire-str.md#关联
- ← recall/build/wire-protocol-gate-is-failfast.md#案例
- ← recall/domain/five-wire-protocols-anchor.md#关联

## arch/protocol-wire-str.md#陷阱
- ← core/arch/protocol-wire-str.md#关联
- ← recall/build/wire-protocol-gate-is-failfast.md#案例
- ← recall/domain/five-wire-protocols-anchor.md#关联

## arch/protocol-wire-str.md#正解
- ← core/arch/protocol-wire-str.md#关联
- ← recall/build/wire-protocol-gate-is-failfast.md#案例
- ← recall/domain/five-wire-protocols-anchor.md#关联

## arch/protocol-wire-str.md#案例
- ← core/arch/protocol-wire-str.md#关联
- ← recall/build/wire-protocol-gate-is-failfast.md#案例
- ← recall/domain/five-wire-protocols-anchor.md#关联

## arch/protocol-wire-str.md#适用
- ← core/arch/protocol-wire-str.md#关联
- ← recall/build/wire-protocol-gate-is-failfast.md#案例
- ← recall/domain/five-wire-protocols-anchor.md#关联

## arch/protocol-wire-str.md#关联
- ← core/arch/protocol-wire-str.md#关联
- ← recall/build/wire-protocol-gate-is-failfast.md#案例
- ← recall/domain/five-wire-protocols-anchor.md#关联
- → [[protocol-wire-str]]

## arch/stream-buf-unified-cap.md#关联
- → [[hot-path-buffers]]
- → [[stream-buf-no-batching]]

## db/crash-safe-db-split.md#Cross-ref
- → [[auto-fix-downgrade-34]]

## db/sqlite-read-cache-config.md#SQLite 只读缓存定值
- ← recall/db/sqlite-cache-residency-probe-method.md#页缓存常驻量探针

## db/sqlite-read-cache-config.md#关联
- ← recall/db/sqlite-cache-residency-probe-method.md#页缓存常驻量探针
- → [[sqlite-cache-residency-probe-method]]

## domain/peak-multiplier-symmetry.md#硬约则
- ← recall/domain/time-tiers-apply-idiom.md#关联

## domain/peak-multiplier-symmetry.md#禁用
- ← recall/domain/time-tiers-apply-idiom.md#关联

## domain/peak-multiplier-symmetry.md#关联
- ← recall/domain/time-tiers-apply-idiom.md#关联
- → [[resolve-price-now-ms]]
- → [[time-tiers-apply-idiom]]

## domain/resolve-price-now-ms.md#resolve_price 末位 now_ms 传值约定
- ← core/domain/peak-multiplier-symmetry.md#关联
- ← recall/domain/bundled-models-fallback.md#关联
- ← recall/domain/time-tiers-apply-idiom.md#关联

## i18n/i18n-key-sync-8lang.md#硬约则
- ← core/i18n/zh-hans-literal-sync.md#关联

## i18n/i18n-key-sync-8lang.md#验收
- ← core/i18n/zh-hans-literal-sync.md#关联

## i18n/i18n-key-sync-8lang.md#禁用
- ← core/i18n/zh-hans-literal-sync.md#关联

## i18n/i18n-key-sync-8lang.md#关联
- ← core/i18n/zh-hans-literal-sync.md#关联
- → [[zh-hans-literal-sync]]

## i18n/zh-hans-literal-sync.md#MUST 硬约束
- ← core/i18n/i18n-key-sync-8lang.md#关联

## i18n/zh-hans-literal-sync.md#禁（多 locale 命名空间统一）
- ← core/i18n/i18n-key-sync-8lang.md#关联

## i18n/zh-hans-literal-sync.md#验收
- ← core/i18n/i18n-key-sync-8lang.md#关联

## i18n/zh-hans-literal-sync.md#关联
- ← core/i18n/i18n-key-sync-8lang.md#关联
- → [[i18n-key-sync-8lang]]
- → [[locale-deadkey-cleanup-ownership]]

## perf/hot-path-buffers.md#mpsc 热路径丢弃分支先查 capacity 再决定是否深拷贝
- ← core/arch/stream-buf-unified-cap.md#关联
- ← recall/ops/logging-queue-capacity-tuning.md#日志队列 capacity 定值方法：从采样均值反推

## perf/hot-path-buffers.md#热点判定维度：调用频次优先于字节量
- ← core/arch/stream-buf-unified-cap.md#关联
- ← recall/ops/logging-queue-capacity-tuning.md#日志队列 capacity 定值方法：从采样均值反推

## perf/hot-path-buffers.md#热点判定维度：调用频次优先于字节量
- ← core/arch/stream-buf-unified-cap.md#关联
- ← recall/ops/logging-queue-capacity-tuning.md#日志队列 capacity 定值方法：从采样均值反推

## perf/stream-buf-no-batching.md#
- ← core/arch/stream-buf-unified-cap.md#关联

## proxy/auto-disable-401-403-402.md#硬约则
- ← core/proxy/http-client-no-env-proxy.md#关联

## proxy/auto-disable-401-403-402.md#触发条件
- ← core/proxy/http-client-no-env-proxy.md#关联

## proxy/auto-disable-401-403-402.md#禁用
- ← core/proxy/http-client-no-env-proxy.md#关联

## proxy/auto-disable-401-403-402.md#关联
- ← core/proxy/http-client-no-env-proxy.md#关联
- → [[http-client-no-env-proxy]]
- → [[mock-platform-short-circuit]]

## proxy/http-client-no-env-proxy.md#MUST 硬约束
- ← core/proxy/auto-disable-401-403-402.md#关联

## proxy/http-client-no-env-proxy.md#反例
- ← core/proxy/auto-disable-401-403-402.md#关联

## proxy/http-client-no-env-proxy.md#症状差异（为何 502 路径不触发）
- ← core/proxy/auto-disable-401-403-402.md#关联

## proxy/http-client-no-env-proxy.md#验证
- ← core/proxy/auto-disable-401-403-402.md#关联

## proxy/http-client-no-env-proxy.md#关联
- ← core/proxy/auto-disable-401-403-402.md#关联
- → [[auto-disable-401-403-402]]

## proxy/wire-protocol-whitelist-sync.md#MUST 硬约束
- ← recall/domain/endpoint-cross-protocol-fallback.md#案例

## proxy/wire-protocol-whitelist-sync.md#反例
- ← recall/domain/endpoint-cross-protocol-fallback.md#案例

## proxy/wire-protocol-whitelist-sync.md#触发场景
- ← recall/domain/endpoint-cross-protocol-fallback.md#案例

## proxy/wire-protocol-whitelist-sync.md#适用
- ← recall/domain/endpoint-cross-protocol-fallback.md#案例

## proxy/wire-protocol-whitelist-sync.md#关联
- ← recall/domain/endpoint-cross-protocol-fallback.md#案例
- → [[five-wire-protocols-anchor]]
- → [[reasoning-content-as-text-block]]

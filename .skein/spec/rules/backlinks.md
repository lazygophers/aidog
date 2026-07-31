# SKEIN rules 关联表 (A-MEM-lite 正反链)

章节粒度: 规则 id = `<类目>/<主题>.md#<规则标题>`; `←` 入链 (谁引用本条) / `→` 出链 (本条引用谁)。无条目 = 孤立候选。

## arch/mock-platform-bypasses-forward-pipeline.md#mock 平台绕开真实转发流水线，无法验证 finish.rs 挂载的 cap/累积逻辑
- ← recall/domain/prd-acceptance-consistency-check.md#PRD 验收标准与约束互容性检查

## arch/mock-platform-bypasses-forward-pipeline.md#硬约束
- ← recall/domain/prd-acceptance-consistency-check.md#PRD 验收标准与约束互容性检查

## arch/mock-platform-bypasses-forward-pipeline.md#关联
- ← recall/domain/prd-acceptance-consistency-check.md#PRD 验收标准与约束互容性检查

## perf/hot-path-buffers.md#mpsc 热路径丢弃分支先查 capacity 再决定是否深拷贝
- ← core/arch/stream-buf-unified-cap.md#关联
- ← recall/ops/logging-queue-capacity-tuning.md#日志队列 capacity 定值方法：从采样均值反推
- ← recall/proxy/sse-chunk-stateless-defect.md#关联

## perf/hot-path-buffers.md#热点判定维度：调用频次优先于字节量
- ← core/arch/stream-buf-unified-cap.md#关联
- ← recall/ops/logging-queue-capacity-tuning.md#日志队列 capacity 定值方法：从采样均值反推
- ← recall/proxy/sse-chunk-stateless-defect.md#关联

## perf/hot-path-buffers.md#热点判定维度：调用频次优先于字节量
- ← core/arch/stream-buf-unified-cap.md#关联
- ← recall/ops/logging-queue-capacity-tuning.md#日志队列 capacity 定值方法：从采样均值反推
- ← recall/proxy/sse-chunk-stateless-defect.md#关联

## perf/stream-buf-no-batching.md#关联
- → [[sse-chunk-stateless-defect]]
- → [[stream-buf-unified-cap]]

## perf/stream-buf-no-batching.md#关联
- → [[sse-chunk-stateless-defect]]
- → [[stream-buf-unified-cap]]

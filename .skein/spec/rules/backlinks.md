# SKEIN rules 关联表 (A-MEM-lite 正反链)

章节粒度: 规则 id = `<类目>/<主题>.md#<规则标题>`; `←` 入链 (谁引用本条) / `→` 出链 (本条引用谁)。无条目 = 孤立候选。

## perf/hot-path-buffers.md#mpsc 热路径丢弃分支先查 capacity 再决定是否深拷贝
- ← core/arch/stream-buf-unified-cap.md#关联
- ← recall/proxy/sse-chunk-stateless-defect.md#关联

## perf/stream-buf-no-batching.md#关联
- → [[sse-chunk-stateless-defect]]
- → [[stream-buf-unified-cap]]

## perf/stream-buf-no-batching.md#关联
- → [[sse-chunk-stateless-defect]]
- → [[stream-buf-unified-cap]]

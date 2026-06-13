# 中间件 C3 — 出站规则执行 + 流式逐块

> 注：熔断器已从本 child 移出，归入新 parent 树 `06-13-group-scheduling-breaker`（group 功能块）。本 child 不再含熔断器。

Parent: `06-13-request-response-middleware` — 8 类请求/响应中间件规则引擎。共享契约见 `../06-13-request-response-middleware/design.md`。

## Goal

在 proxy 出站链路（forward 返回后）挂载响应类规则：响应覆写/整流/错误规则检测；错误规则分类产出 retryable/non-retryable 喂给**现有重试编排**；流式 SSE 逐块改写脱敏/覆写/敏感词。完成后：上游响应含密钥时回客户端为脱敏版(含流式)；上游错误按规则分类驱动重试决策。

## What I already know
- 依赖 **C1**（MiddlewareEngine；MiddlewareSettings 仅 enabled + type_toggles，熔断器已剔除）。
- 与 **C2** 同改 proxy.rs → C2 完成后再实施（串行，避免冲突）。
- 出站挂载点 = forward 返回后/回客户端前；流式有 StreamAggregator 旁路（memory streaming-sse-log-aggregation）。
- 现有重试：多平台重试 + 401/403 auto_disabled + 指数退避（memory platform-retry-failover）。
- 熔断器**不在本 child**：error_rule 只产出 retryable/non-retryable 标记喂现有重试；熔断逻辑由 group 树消费这些标记（解耦）。
- 出站流程/流式逐块细节见 parent design.md「出站」（熔断段忽略）。

## Deliverable 矩阵
| ID | 交付物 | 类型 | 独立验收 | 优先级 |
| --- | --- | --- | --- | --- |
| D3.1 | middleware.rs 出站 apply + error_rule 分类 | diff | 分类/覆写单测 | P0 |
| D3.2 | proxy.rs 出站挂载 + 重试编排接线（retryable 标记）+ 流式逐块改写 | diff | 集成测试(含流式) | P0 |

## Requirements
- R8 出站：response_override/redaction/content_filter 改写非流式 body；error_rule 状态码非 2xx 时分类 + 标记 retryable/non-retryable + override_status/body。
- error_rule 产出的 non-retryable 标记 → 现有重试编排立即返回不重试；retryable → 继续换候选。（不引入熔断器，熔断在 group 树）
- R10 流式 SSE 逐块：每 chunk 应用 mask/override/sensitive；error 按首块/状态码判定；跨块边界漏匹配记为已知限制。
- R7 fail-open；总开关/子开关 OFF 跳过。

## Acceptance Criteria
- [ ] 上游响应含密钥时回客户端为脱敏版（非流式 + 流式各一测）。
- [ ] 上游错误按 error_rule 正确分类，non-retryable 立即返回不重试，retryable 换候选。
- [ ] `cargo test && cargo clippy --all-targets -- -D warnings` 全绿。

## Definition of Done
- Requirements 全实现 + AC 勾选；变更自动暂存提交；error_rule↔重试接线方式落 cortex。

## Out of Scope
- **熔断器**（移至 group 调度树 `06-13-group-scheduling-breaker`）。
- 入站规则（C2）；内置规则内容（C4）；UI（C5）；跨块滑窗匹配（已知限制，后续）。

## Technical Notes
- 改 proxy.rs 出站段 + SSE 转发 + middleware.rs。
- **必须 C1 + C2 完成后开工**（依赖引擎 + 同改 proxy.rs 串行）。
- error_rule 仅产出标记，熔断消费方在 group 树 —— 两树通过"retryable/non-retryable 标记 + auto_disabled 状态"解耦协作，design 协同细节见 group 树 design.md。

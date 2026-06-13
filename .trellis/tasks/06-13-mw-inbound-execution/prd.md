# 中间件 C2 — 入站规则执行

Parent: `06-13-request-response-middleware` — 8 类请求/响应中间件规则引擎。共享契约见 `../06-13-request-response-middleware/design.md`。

## Goal

在 proxy 入站链路（chat_req 就绪后）挂载 5 类入站规则执行：请求过滤器 → 敏感词 → 脱敏 → 内容过滤(密钥/邮箱) → 动态注入；命中按 action 脱敏改写/拦截拒绝/告警；拦截类立即返回并写审计日志不计费；全程 fail-open。完成后：含密钥/邮箱请求上游收到脱敏版；含敏感词请求被拦截返回 + proxy_log 写 blocked 记录。

## What I already know
- 依赖 **C1** 的 MiddlewareEngine/作用域解析/MiddlewareSettings（C1 完成后再实施）。
- 入站挂载点 proxy.rs ~638 行（parse 后）；全局/group 规则路由前应用，platform 规则候选选定后应用。
- proxy_log blocked 字段需核对 db.rs schema（无则 C2 内补列）。
- 执行流程/顺序/fail-open 细节见 parent design.md「入站」。

## Deliverable 矩阵
| ID | 交付物 | 类型 | 独立验收 | 优先级 |
| --- | --- | --- | --- | --- |
| D2.1 | middleware.rs 入站 apply（5 类规则动作） | diff | 各动作单测 | P0 |
| D2.2 | proxy.rs 入站挂载 + 拦截返回 + 审计写入 | diff | 集成测试 | P0 |

## Requirements
- R6 入站顺序：request_filter→sensitive_word→redaction→content_filter→dynamic_injection。
- block 命中：写 proxy_log(blocked_by/blocked_reason)，不计费，立即返回 4xx。
- mask 命中：原地改写 chat_req.messages/system。inject：按 inject_mode。warn：tracing 告警。
- R7 fail-open：单条规则异常放行 + 记日志。
- 总开关 OFF 或该 rule_type 子开关 OFF → 跳过。

## Acceptance Criteria
- [ ] 含密钥/邮箱请求发出后上游收到脱敏版（集成/单测）。
- [ ] 含敏感词请求被拦截返回错误 + proxy_log 写 blocked、不计费。
- [ ] 规则异常时 fail-open 不阻断主链路。
- [ ] `cargo test && cargo clippy --all-targets -- -D warnings` 全绿。

## Definition of Done
- Requirements 全实现 + AC 勾选；变更自动暂存提交；非平凡发现落 cortex。

## Out of Scope
- 出站/响应类规则（C3）；内置规则内容（C4）；UI（C5）。

## Technical Notes
- 改 proxy.rs 入站段 + middleware.rs；可能补 proxy_log blocked 列。
- **必须 C1 完成后开工**（依赖 MiddlewareEngine）。

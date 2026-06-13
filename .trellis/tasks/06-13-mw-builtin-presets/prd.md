# 中间件 C4 — 内置预设规则集

Parent: `06-13-request-response-middleware` — 8 类请求/响应中间件规则引擎。共享契约见 `../06-13-request-response-middleware/design.md`。

## Goal

提供开箱即用的内置预设规则：密钥(常见 API key 模式)/邮箱/手机号脱敏正则 + 默认 error_rules 分类规则；is_builtin=1，首次启动 seed，默认 enabled，可禁用不可硬删。完成后：全新 db 首启即含内置规则，密钥/邮箱默认被脱敏。

## What I already know
- 依赖 **C1**（middleware_rule 表 + CRUD）；与 C1 同改 db.rs → C1 完成后实施。
- 参考仓库无现成密钥/邮箱内容过滤代码，需自建（research 已记）。
- error_rules category 集：prompt_limit/content_filter/pdf_limit/thinking_error/parameter_error/invalid_request/cache_limit（research）。

## Deliverable 矩阵
| ID | 交付物 | 类型 | 独立验收 | 优先级 |
| --- | --- | --- | --- | --- |
| D4.1 | 内置脱敏/内容过滤正则集（密钥/邮箱/手机） | diff | 命中样例文本单测 | P0 |
| D4.2 | 默认 error_rules 集 | diff | 命中样例错误单测 | P0 |
| D4.3 | db.rs seed 逻辑（首启 insert，幂等） | diff | 首启 seed + 重启不重复 | P0 |

## Requirements
- R11 内置预设 is_builtin=1 默认 enabled；seed 幂等（已存在跳过/更新，不重复插）。
- 密钥正则覆盖常见模式（sk-/Bearer/AKIA/ghp_ 等）；邮箱/手机标准正则。
- 默认 error_rules 覆盖上述 category 常见上游错误消息模式。
- 内置规则可被用户禁用，不可硬删（软删或 UI 隐藏删除按钮，与 C5 约定）。

## Acceptance Criteria
- [ ] 全新 db 首启 seed 内置规则（集成/单测）。
- [ ] 内置密钥/邮箱/手机正则命中样例文本（单测）。
- [ ] 默认 error_rules 命中样例错误消息（单测）。
- [ ] 重启不重复 seed（幂等单测）。
- [ ] `cargo test && cargo clippy --all-targets -- -D warnings` 全绿。

## Definition of Done
- Requirements 全实现 + AC 勾选；变更自动暂存提交；内置正则清单落 cortex。

## Out of Scope
- 规则执行逻辑（C2/C3）；UI（C5）。

## Technical Notes
- 改 db.rs(seed) + 可能 middleware.rs(内置常量)。
- **必须 C1 完成后开工**。

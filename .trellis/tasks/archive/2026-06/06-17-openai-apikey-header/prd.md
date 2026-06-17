# openai 协议 api-key 头鉴权适配（小米 token-plan）

## Goal

让 aidog 在转发到「小米 MiMo token-plan 的 openai 端点」时用官方要求的 `api-key:` 头鉴权（而非默认 `Authorization: Bearer`），使小米 coding plan 的 openai 端点可用。

## What I already know

- 小米 token-plan openai 官方示例用 `--header "api-key: $MIMO_API_KEY"`（归档 research `06-17-xiaomi-coding-plan/research`）；aidog 默认 openai→`Authorization: Bearer`，可能被拒。anthropic 端点用 `x-api-key` 已兼容可用。
- 鉴权头硬编在 `src-tauri/src/gateway/proxy.rs` 3 处：`apply_default_headers`(:3114)、`apply_claude_code_family_headers`(:3144)、`apply_codex_family_headers`(:3170)，openai 分支均 `Authorization: Bearer`。另 `apply_models_auth`(:2266) 管 /models 拉取。
- ⚠️ Bearer 被拒**未实证**（无 tp- key 实测），exec 研究仅据官方示例推断。

## Decision (ADR-lite)

- **Context**: 小米 token-plan openai 用 `api-key:` 头，aidog 默认 Bearer。
- **Decision**: **openai 协议上游请求同时发 `Authorization: Bearer` + `api-key:` 双头**（保留 Bearer，叠加 api-key），不做 base_url 检测、不加配置字段；**直接适配**不等实证（小米官方明写 api-key）。
- **Consequences**: 所有 openai 平台都多带一个 `api-key` 头；HTTP 服务器通常忽略未知头，风险低；满足小米同时不破坏其他平台（如个别上游拒未知头，回退方案改 base_url 子串定向）。

## Requirements (evolving)

- [ ] 小米 token-plan openai 端点转发时带 `api-key:` 头。

## Acceptance Criteria (evolving)

- [ ] 命中小米 token-plan openai base_url → 上游请求含 api-key 头。
- [ ] 不影响其他 openai 平台（仍 Bearer）。
- [ ] cargo build/clippy/test 绿。

## Out of Scope

- 其他平台的非标鉴权（仅小米）。

## Technical Notes

- 触点：proxy.rs 3 个 apply_*_headers + 可能 apply_models_auth。
- 参考记忆：xiaomi-mimo-token-plan-no-api、url-construction-rule。

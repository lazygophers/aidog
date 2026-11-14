# ADR 0013: embedding / rerank 请求纳入余额与预算预估（issue #61 评审考证）

## 背景

PR #61 之前，`POST /v1/embeddings` 与 `POST /v1/rerank` 的成功转发不触发余额扣减，也不进手动预算预估——平台余额与预算只被 chat 请求消耗。评审要求先确认「不扣余额」是否有意设计，有意则文档写明。

## 考证结论

**非有意设计，是实现缺口。** 证据：

1. 反证注释：`gateway/proxy/tools.rs` 独立搜索路径的注释写着「按次费入余额预估：独立搜索 token 全 0，spawn_estimate 凭按次费绕过『无 token 跳过』」——同一时期的设计意图是**能进预估链的都进**，embedding / rerank 是漏了，不是排除。
2. 引入两条链路的 commit（`f62d4b69f` feat(proxy): embedding 请求转发链路；`9b039d5ff` feat(gateway): /v1/rerank 槽位转发链路与按次计价）的 message 均无「不扣余额」的设计说明。
3. ADR / GLOSSARY / CLAUDE.md 在 PR #61 之前均无该行为的任何记录。

## 决策

embedding / rerank 成功请求与 chat 同链路：经 `spawn_estimate` → `estimate_after_request` 入平台余额扣减与手动预算扣减，口径与 `est_cost` 一致（`aux_model.rs` 调用点；rerank 的 `search_units` 作按次计量次数，缺次按 1 次）。

## 后果

- 挂余额的平台，embedding / rerank 用量会持续扣减预估余额（此前恒不动）。
- 命中校准阈值时，这些请求同样触发余额真查校准。
- 若日后发现真实上游对这些请求不计量（不扣真实余额），以「预估口径与计费口径不一致」重开评估。

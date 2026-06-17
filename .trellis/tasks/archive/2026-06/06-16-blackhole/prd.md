# PRD: 熔断候选空回退透传（单平台不 blackhole）

## 背景 / 问题

诊断自 `~/.aidog/aidog.db` proxy_log，group `glm-coding-plan-auto`（单平台 GLM，routing_mode=failover）：

- 06-15 10:09–10:16 出现 6 条 `400 route error: no available platform (...circuit-broken)`。
- 因果链：GLM 频繁 429/5xx → `proxy.rs:1250`（`code>=500 || code==429`）每次计熔断失败 → 熔断 Open → router `select_candidates_ctx` 踢掉**唯一**平台 → 返回 `Err("no available platform")` → `proxy.rs:914` 返回 400。
- 用户表现："用着用着就断开了且说连不上"。

根因：熔断器语义是"在多个健康平台间摘除坏的"，但**无可切换目标时**（单平台分组 / 多平台全坏）Open 会把全部流量打到黑洞，且丢失上游真实 429/5xx + `retry-after`，客户端无法正确退避。

## 目标

熔断过滤后候选为空时，**回退忽略熔断重新选一次**，保证至少 1 个候选，把上游真实状态码 / body / headers（含 retry-after）原样透传给客户端。

## 方案（已与用户确认：候选空回退透传）

在 `router.rs select_candidates_ctx`：
- 第 1 步分桶时，熔断维度（`Admission::Reject`）踢出的候选**单独记一份**（区别于 auto_disabled / 手动 disabled 踢出的）。
- 第 3 步合并后若 `ordered` 为空，且为空的原因是熔断（而非全部 auto_disabled / disabled）→ 回退：把被熔断踢出的候选按路由序重新纳入，标记本轮为"熔断旁路探活"。
- 仍全空（真无 enabled 平台）→ 保持原 `Err`。

## 验收标准

1. 单平台分组、平台熔断 Open 时，请求不再返回 400 "no available platform"，而是转发到该平台，把上游真实状态码（429/5xx）+ body + headers 透传给客户端。
2. 多平台场景行为不变：有健康平台时优先健康平台，熔断的排末尾/不选（仅在全坏时回退）。
3. `cargo test` 全绿；新增针对"熔断踢空 → 回退非空"的单测。
4. `cargo clippy` 无 warning。

## 非目标

- 不改 429 是否计熔断（保留计数，仅改"踢空回退"）。
- 不改 GLM 上游本身的 502/500/529（上游侧，非 aidog 可控）。

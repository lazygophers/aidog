# PRD — 402 自动禁用免 purge + proxy 错误记入平台状态

## 背景

平台卡片已有 `last_error` 红色徽章（commit afcd6fb，DB 持久化非实时取，见 [[platform-last-error-persisted]]）。现状缺口:

1. **402（余额不足）不触发自动禁用** —— 仅 401/403 自动禁用（`non_success.rs:57`）。402 会反复试探。
2. **一键清理（purge）删所有 auto_disabled 不分原因**（`platform_lifecycle.rs` SQL `status = 'auto_disabled'`）—— 402 因余额耗尽被禁用属可充值恢复，误删后需重建。
3. **last_error 存完整 body** —— 形如 `HTTP 429: {"type":"error","error":{"message":"已达到 Token Plan..."}}`，噪声大。
4. **卡片无直观健康指示** —— 仅文本徽章，不够醒目。

## 需求（用户确认）

### R1 — 402 触发自动禁用（指数退避）
- `non_success.rs` 自动禁用触发条件由 `code == 401 || code == 403` 扩为含 `402`。
- 退避策略复用现有 `set_platform_auto_disabled`（指数退避，与 401/403 一致）。用户明确「无论什么错误都是指数退避」。
- 熔断计数不变：402 属 4xx，不计熔断（同 401/403），仅 auto_disabled + inflight-1。

### R2 — purge 只删 401/403 或已过期
- `purge_auto_disabled_platforms` 全局 + 分组级 SQL：auto_disabled 平台仅当 `last_error` 是 401/403 才删；其余 auto_disabled（402 等可恢复）保留。
- 已过期平台（`expires_at`）仍照删（与现状一致）。
- 判据（用户明确「根据最近错误判断」）：`last_error LIKE 'HTTP 401%' OR last_error LIKE 'HTTP 403%'`。
- 新 SQL 谓词:
  ```sql
  (status = 'auto_disabled' AND (last_error LIKE 'HTTP 401%' OR last_error LIKE 'HTTP 403%'))
  OR (expires_at > 0 AND expires_at < ?now)
  ```

### R3 — last_error 优先存 message，不存完整 body
- 新增 helper：给定错误 body 字符串，尝试 JSON 解析提取 `error.message`（嵌套）或顶层 `message`，命中则 last_error = `HTTP {code}: {message}`；未命中回退现有 `truncate_attempt_error`。
- 应用于 `non_success.rs` 写 last_error 处。连接失败/空 2xx 站点保持现状（无 body 可解）。
- 例：`HTTP 429: 已达到 Token Plan 用量上限：请升级 Token Plan 套餐或购买积分补充用量。 (2056)`。
- 「其他类似的也一样」「有 message 只展示 message」= 通用 message 提取，不限 429。

### R4 — 平台卡片健康状态点
- `PlatformCard.tsx` 平台名旁加健康指示点（绿/黄/红），综合最近状态:
  - **红** = key 失效（status auto_disabled 且 last_error 401/403）。
  - **黄** = 有 last_error 但可恢复（402/429/5xx/连接失败/其他），含 402 auto_disabled。
  - **绿** = enabled 且无 last_error。
- title 复用 last_error 提示（已有 i18n key）。判定纯前端派生，不加后端字段。

### R5 — 429 配额耗尽自动禁用（区分限流）
- 实证（request 40bb761d）：单次请求重试 9 次全打在配额耗尽的 coding-plan 平台（小米 MiMo / MiniMax），耗时 5920ms 终落 GLM-Self 400。根因：429 配额耗尽平台未隔离，反复试探。
- **必须按 message 文本分两类（不能按 status 或 type 一刀切）**:
  - **429 配额耗尽** → 同 402 处理：`set_platform_auto_disabled`（指数退避），免 purge。markers（message 含任一，大小写不敏感）：`quota exhausted` / `用量上限` / `Token Plan` / `insufficient` / `余额` / `积分`。
  - **429 限流（transient）** → 维持现状：`record_failure`（熔断）+ failover 重试，**不 auto_disabled**。markers：`too many requests` / `rate limit`（且不含配额 markers）。
  - 默认（无 marker 命中）→ 保守按**限流**处理（不禁用），避免误杀。
- ⚠️ 关键陷阱：MiniMax 配额耗尽响应 `type` 也是 `rate_limit_error`，故分类**只能看 message 文本**，禁按 `error.type` 判。
- 熔断计数：配额耗尽类不计熔断（同 402，仅 auto_disabled）；限流类仍计熔断（现状不变）。
- 新增 helper `classify_429(message) -> bool`（true=配额耗尽需禁用），带单测覆盖三类样本（quota exhausted / 用量上限 / Too many requests）。
- 注：上轮遗留「pre-flight 预筛已知耗尽平台免试探」**不在本 task 范围**——auto_disabled 后续请求自然跳过即覆盖主要痛点；预筛留作后续独立优化。

## 范围与边界
- 改动文件：`non_success.rs`（R1+R3+R5）、`platform_lifecycle.rs`（R2）、`PlatformCard.tsx`（R4）、可能新增 helper（R3 message 提取 + R5 classify_429，放 proxy 模块）。
- 不加新 DB 列（R2 用 last_error 既有列；R4 纯前端派生）。
- aidog 自身 402（manual_budget 耗尽，`forward.rs:177`）是本地响应非上游，不经 non_success，不受影响。

## 验收
- `cargo test` + `cargo clippy` 全绿；新增 message 提取 helper 带单测（JSON 嵌套/顶层/非 JSON 回退）。
- purge SQL 单测或手验：402 auto_disabled 不被删，401/403 被删，过期被删。
- `yarn build` 通过；健康点三色按状态正确渲染。
- i18n 无新增裸 key（health dot title 复用现有）。

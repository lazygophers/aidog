---
updated: 2026-07-01
rewrite-version: 1
authored-by: trellisx-spec
mode: sediment
---

# Platform Error Handling

何时被读: 改 proxy 失败处理 / 加平台 / 调 auto_disable / 熔断 / purge / last_error 时
谁读: main / sub-agent
不遵守的代价: 限流平台反复试探拖慢请求 / 可恢复平台被误删需重建 / 误把限流当配额隔离

来源 task: 06-30-platform-402-autodisable-error-status

---

## C1 — auto_disable 触发状态码 (MUST)

`non_success.rs` handle_non_success 中, 上游非 2xx 仅以下触发 `set_platform_auto_disabled`(指数退避):

- `code == 401 || code == 403`(鉴权失败/key 问题)
- `code == 402`(余额不足, 可充值恢复)

429(无论配额耗尽还是限流)**不**触发 auto_disable: classify_429 仅用于 C3 熔断分类, 不进 auto_disable 树。
其它任何状态码(含 404/405/429)**禁**自动禁用, 仅按 failover 换下个候选重试。

验证: `grep -n 'code == 401 || code == 403 || code == 402' src-tauri/src/gateway/proxy/non_success.rs` 必须命中, 且同行无 `|| is_429_quota_exhausted`。

## C2 — 429 分类只看 message 文本 (MUST NOT 按 error.type)

`classify_429(message) -> bool`(retry.rs)区分 429:

- **配额耗尽**(true → 同 402 处理): message(大小写不敏感)含任一 marker: `quota exhausted` / `用量上限` / `token plan` / `insufficient` / `余额` / `积分`。
- **限流 transient**(false): 无 marker 命中 → 默认按限流(保守不禁用, 避免误杀)。

🔴 **禁按 `error.type` 判**: MiniMax 配额耗尽响应 `type` 也是 `rate_limit_error`, 只能按 message 文本分类。

验证: `grep -n 'fn classify_429' src-tauri/src/gateway/proxy/retry.rs` + 单测覆盖 quota/限流两类。

## C3 — 熔断与 auto_disable 解耦 (MUST)

熔断计数(`record_failure` vs `record_ignored`)按下表:

| 错误 | 熔断 | auto_disable |
| --- | --- | --- |
| 5xx | record_failure | 否 |
| 429-限流 | record_failure | 否 |
| 429-配额 | record_ignored(不计熔断) | 否(统一走 failover, 不禁用) |
| 401 / 403 / 402 | record_ignored(不计熔断) | 是 |
| 其它 4xx(404/405 等) | record_ignored | 否 |

走 auto_disable 的(401/403/402)**不参与熔断**, 仅 inflight-1。

## C4 — purge 只删 401/403 或已过期 (MUST)

`purge_auto_disabled_platforms`(platform_lifecycle.rs)全局 + 分组级 SQL 谓词:

```sql
(status = 'auto_disabled' AND (last_error LIKE 'HTTP 401%' OR last_error LIKE 'HTTP 403%'))
OR (expires_at > 0 AND expires_at < ?now)
```

402 / 429-配额等可充值恢复的 auto_disabled **必保留**(不被一键清理误删); 401/403(key 失效需重建)与过期平台照删。判据基于 `last_error` 既有列, **禁**为此加新 DB 列。

验证: `test_platform_lifecycle.rs::purge_keeps_recoverable_auto_disabled`。

## C5 — last_error 优先存 message 不存完整 body (MUST)

写 `set_platform_last_error` 前用 `extract_error_message(body)`(retry.rs)提取人类可读 message:
嵌套 `error.message` → 顶层 `message` → 命中则 `last_error = HTTP {code}: {message}`;
未命中(非 JSON / 无字段 / 空白)回退 `truncate_attempt_error` 摘要。连接失败/空 2xx 等无 body 站点保持现状。

**历史数据修复**: 037 加列时(afcd6fb)写入路径未走 extract_error_message, 落库的是 `HTTP {code}: {完整 body}`。
后续 b9f82ed 才接入 C5 规则。037 与接入之间窗口内写入的行需 Migration 039 一次性重提(`schema_late.rs::reextract_legacy_last_error`),
仅对 body 含 `error.message` / 顶层 `message` 的行重写, 其余(纯文本 / 非 JSON / 已提取过)保留。禁再次加新迁移清这类残留——039 幂等, 已覆盖。
(编号 038 被 group-env-vars 任务先占, 本迁移顺延 039。)

## C6 — stream 字段单向性：禁用 unwrap_or(false) 区分漏发与显式非流式 (MUST)

**背景**：DB 全库实证（2026-07-02）—— 客户端（Claude Code）stream 字段是**单向**的：
- 流式：显式发 `stream:true`
- 非流式：**省略字段**（`is_none`），**从不发 `stream:false`**

全库零显式 `stream:false`（539 条漏发非流式 vs 0 显式 false）。

**契约**：
- proxy 任何 stream 判定逻辑**禁用** `chat_req.stream.unwrap_or(false)` 区分「客户端漏发」与「客户端显式非流式」—— 两者在真实流量里**恒混同**（漏发占 100%，显式 false 占 0%）。
- 凡需区分两者的门控（如 hoist 规整、规整跳过），`Some(false)` 门控**永假** = 无条件禁用该分支。改用其他结构信号（messages 数 / role=system 块数 / body 大小）做门控。
- `is_stream = chat_req.stream.unwrap_or(false)` 仅用于「流式 vs 非流式」二分（响应处理路径选择），**不得**作为「客户端意图」语义判定。

**反例**：方案 B `if chat_req.stream == Some(false) { hoist }` —— DB 实证门控永假，等于禁用 hoist 分支（[hoist-reevaluation.md](../../tasks/07-02-recurring-request-error/research/hoist-reevaluation.md)）。

## C7 — 空流/空body 失败时 response_body MUST 落上游真实首块 (MUST)

**背景**：proxy 流式 peek 判 `EmptyOrError`（上游 200 但流无内容/秒断/立即[DONE]/立即error）或非流式 200 但 body 无效时，原逻辑把 `response_body` 设为占位文案（28 字节 `"200 but empty/invalid stream"`），**上游真实首块未落库** → 全库 1301 条空流 502 无 DB 证据可取证上游返了什么。

**契约**：
- `retry_on_empty_2xx!` 触发时（流式 peek 判空 / 非流式 body 无效），`log.response_body` **MUST** 落上游真实文本（`peek_text` / `resp_str`）截断后内容，**禁用纯占位文案**。
- 截断策略：4KB 上限（`PEEK_MAX_BYTES=64KB` 必截），UTF-8 字符边界（`char_indices` 防切断多字节），尾部加 `…[truncated N bytes]`。
- 空串兜底：上游真返回空（`peek_text`/`resp_str` 为空）时，回退占位文案。
- 目的：下次间歇性空流复现时自动留 DB 证据，数据驱动定位上游行为。

**实现**：`forward.rs:369-407` 宏 + `retry.rs:115-136 truncate_peek_text`（commit 24883d2e）。

# PRD: 修复流式 proxy_log token=0

## Bug
流式请求的 proxy_log input/output/cache_tokens 恒记 0。

## 根因
proxy.rs 流式分支在返回 stream Response **之前** `upsert_log`（约 :505），此时 axum 尚未消费 stream、`tokens_acc`/`tokens_out` AtomicI32 = 0 → 日志 token=0。token 仅在流消费完（`[DONE]`，:574 闭包）才累加完整 —— 预估已正确用最终值（:582 est_in.load），但**日志 upsert 提前**。

## 修复
在 `[DONE]` 闭包内（:574，spawn_estimate 旁）**再次 upsert_log** 更新 token：log 副本 clone 进闭包，[DONE] 时用最终 `tokens_acc/tokens_out/cache` load 值 + `upsert_log`（INSERT OR REPLACE 同 log.id 覆盖 token 字段）。复用 est_fired AtomicBool 防重复（或独立标志）。非流式分支不受影响。

## 验收
- 流式请求后 proxy_log token 正确（非 0）
- 不重复 upsert（[DONE] 一次）；非流式不受影响
- cargo build + test

## Subtask
- ST1: [DONE] 闭包 upsert_log 更新 token
- ST2: 验证（流式 token 非 0 + 非流式无回归）

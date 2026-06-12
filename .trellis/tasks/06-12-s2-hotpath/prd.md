# S2 后端热路径优化

> Parent: [06-12-p0-p1](../06-12-p0-p1/prd.md) · 依赖 S1（async DB）完成后执行。

## Goal

消除 proxy 热路径的连接重建、同步日志写库、重复解析与冗余内存拷贝，降低延迟与 CPU。功能零回归。

## Requirements

- R1.1 共享单个 reqwest `Client`（存 ProxyState/tauri::manage），超时改 per-request `RequestBuilder.timeout()`；connect_timeout 用 Client 构造默认。删 proxy.rs:706/1115 + lib.rs:412/551 的每请求 builder。
- R1.3 proxy_log 写改 mpsc channel → 后台任务批量 flush，热路径不阻塞（proxy.rs:313 等写点）。
- R1.4 body 单次 parse 缓存 `&Value`（去除 proxy.rs:500+539 双解、722 pretty 再解）；日志用 Bytes 借用替代多次 `.to_vec()`（780/804/1142/1219）。
- R2.1 流式 chunk 行级前缀检查（`data: ` / `[DONE]`）替代逐行 serde parse，仅含 usage 的行才 `from_str`（proxy.rs:891）。
- R2.2 coding-plan delta 锁外算、（async DB 下）短写（estimate.rs:234-250）。

## Acceptance Criteria

- [ ] 热路径无 `Client::builder()` 每请求调用；连接 keep-alive 复用可验证。
- [ ] proxy_log 写不阻塞流式消费；日志最终一致（可接受短延迟，记入文档）。
- [ ] 大 body 转发内存拷贝次数下降；解析仅一次。
- [ ] `cargo build` 0 warning；proxy 转发 + SSE 流 + 日志 + est_cost 行为不变。

## Out of Scope

- DB 异步化本体（S1 已做）。
- 替换 JSON 库 / simd-json。

## Technical Notes

- 遵 backend spec：mock-platform / claude-code-passthrough 拦截点与 header 语义不变。
- log 异步可见延迟为行为变更 → 需记 CLAUDE.md / spec。
- 依赖 S1 的 async DB 接口；S1 未并入不得开工。

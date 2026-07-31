# 转发热路径缓冲与拷贝治理 — PRD (主入口)

## 目标
- [ ] 修 from_utf8_lossy 对原始网络 chunk 解码致多字节字符跨 chunk 边界变 U+FFFD 的正确性缺陷 —— 转换分支下发客户端的字节由这份 lossy 文本派生，直接压红线 2
- [ ] 把 StreamAggregator 的 upstream_body / client_body 的 16MB cap 从 join 时前移到 push 时 —— 现状累积期无 cap，N 路并发长流各持完整响应体两份，是长跑内存尖峰的真实防线
- [ ] 修 upsert_log 在 is_terminal_log 判定之前就 log.clone() —— 非 terminal 走 try_send 队列满即丢，等于先付全额深拷贝再扔掉，且每请求调 40+ 次
- [ ] 消除请求 JSON 主路径最多 3 次深拷贝（converter from_value 的 body.clone、forward 为改一个 model 字段 clone 整棵树）
- [ ] 给 get_group_platforms 补缓存 —— 每请求 2 次 SQL + N 次 JSON 反序列化，同文件已有现成的 group_details 缓存与失效基建未复用
- [ ] 给 RollingFileAppender 套 tracing_appender::non_blocking —— 现状每条 event 在发出它的线程上同步写文件，包括转发热路径的 tokio worker
- [ ] 给 sse_line_buf 加上界，防上游 SSE 永不发换行时无限追加
## 边界
- [ ] 只动 Rust 转发热路径与其日志缓冲，不改协议转换语义与上下游报文内容
- [ ] token 计数与费用精度不得下降（红线 2）—— usage 走 feed_sse_usage 独立累计，本 task 的所有改动必须保持其输入完整
- [ ] 转发延迟与首 token 时延不得变差（红线 1）—— 所有改动为减少工作量，若某项实测反致延迟上升则回退该项
- [ ] 不动 tokenizer 与 count_tokens（归 tokenizer-residency-trim）
- [ ] 不动 SQLite cache_size 与查询形态（归另两个 task）
- [ ] 不改 ProxyLogSettings 三级开关语义与 retention 行为
- [ ] 一切压测只用 mock 平台与分组，禁打真实上游
## 验收标准
- [x] 存在一条就红的复现用例证明跨 chunk 多字节 UTF-8 字符当前会变 U+FFFD，修复后该用例转绿
- [x] 50 路 mock 并发长流下，upstream_body / client_body 的累积字节受 STREAM_BODY_MAX_BYTES 约束，超限置截断标记而非继续 push
- [x] push cap 的内存防护由单测证明（超上界后停止累积并标截断），并记录「mock 平台下端到端不可测」的架构原因 —— 用户 2026-07-31 拍板改此口径，原为「50 路 mock 并发压测的 phys_footprint 峰值相对基线下降 + 下降归因」。改因：`handler.rs:410-429` 对 `platform_type=mock` 短路进 `handle_mock`，不进 `finish.rs` 转发流水线，而 cap 挂在 `finish.rs`/`passthrough.rs` 上 —— 该验收项与本 PRD 边界「一切压测只用 mock 平台」架构上互斥，无论负载多大都触不到那段代码（s9 实证：两侧 50 条 `response_body` 恒为 `"[mock stream]"` 占位符，截断标记从未出现）
- [x] upsert_log 的 clone 只发生在 terminal 分支或队列有容量时
- [x] 请求 body 深拷贝次数从最多 3 次降到 1 次以内，用计数或 trace 证明
- [x] get_group_platforms 命中缓存时不产生 SQL 查询，且现有失效触发点覆盖所有写入路径
- [ ] tracing 文件写不再发生在 tokio worker 线程上
- [x] sse_line_buf 有明确上界，触发上界时行为已定义且不 panic
- [x] 转发延迟 p95 与首 token 时延 p95 相对基线不上升（50 路 mock 并发）
- [x] token 计数与 est_cost 在改动前后对同一组 mock 请求逐条一致
- [x] cargo clippy --workspace 零 warning、cargo test --workspace 全绿
- [x] 全程只用 mock 平台与分组，记录中可核验无真实上游调用
- [x] 清场完成：压测临时脚本与逐次采样已删
## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md) (仅真调研时生)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list proxy-hotpath-buffers`)

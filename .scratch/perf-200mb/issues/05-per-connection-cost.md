# 05 50 路并发流式转发的每连接内存与 CPU 成本

Type: task
Status: open
Blocked by: 01
Parent: [深度性能优化：全进程峰值内存 ≤200MB + 三场景 CPU 下降](../map.md)

## Question

峰值内存是「基线 + 并发数 × 每连接成本」。每连接成本是多少，由什么构成？

用 mock 协议造 1 / 10 / 50 / 100 路并发流式请求，量出内存随并发的斜率，并拆解构成：
- in-flight 请求 / 响应缓冲（注意 memory 已记的 `streaming-snapshot-meta-only`、`symmetric-body-cap` 两条既有结论，确认是否仍生效、有无新漏点）
- reqwest 连接池与 rustls session 缓存
- 协议转换层（`gateway/converter`）在流式路径上的临时分配
- proxy_log 落库路径的缓冲与批量策略（`gateway/db/proxy_log.rs`）
- tokio task 栈与 channel 缓冲

同时抓转发态火焰图，标出热路径上占比 >2% 的栈，特别关注：token 计数、est_cost 计算、序列化 / 反序列化、日志脱敏、SQLite 写入。

**这张票是 task 不是 grilling**：它只负责把数字和栈量出来，怎么改由后续票决。但它会 graduate map 中「异步化边界」那片 fog。

## 验收

- 并发-内存曲线（1/10/50/100 四个点），给出每连接边际成本 MB 数
- 每连接成本的构成拆解表，每项标 file:line
- 转发态火焰图 + 热路径 top-N 栈，标出「可挪出热路径」的候选（不做决定，只标候选）

---
title: hot-path-buffers
category: perf
keywords: [mpsc,capacity,try_send,背压,深拷贝,热路径,TOCTOU,hotspot,frequency,profiling,clone]
status: active
inclusion: auto
anchors: src-tauri/crates/aidog_core/src/gateway/proxy/log.rs
---

## mpsc 热路径丢弃分支先查 capacity 再决定是否深拷贝

mpsc 队列热路径丢弃分支：先 `Sender::capacity() == 0` 判队满再 return，避免为「确定要被丢弃」的消息
（try_send 会因 Full 返回错误）付出昂贵深拷贝构造成本。适用场景：try_send 非阻塞丢弃型背压 + 消息体含
大 String/Vec 等重克隆字段。TOCTOU 权衡：check-then-send 存在极小竞态窗口（多 producer 场景 capacity
检查后被并发填满），可接受——退化为回到原 try_send 路径正常处理 Full/Closed，不引入正确性问题，只是
偶发未省下这次克隆。closed channel 场景不特判（罕见 shutdown 窗口，走原有 match 分支即可）。

## 热点判定维度：调用频次优先于字节量

## 热点判定维度：调用频次优先于字节量

### 核心决策

**热点判定的决定变量是调用频次（每请求 N 次），不是单次操作的字节量。**深拷贝值不值得优化，取决于调用频次与信号噪声比，与报文大小无直接关系。

### 规则

- **高频（热点）**：每请求循环内 40+ 次调用，即使单次只拷贝 100 字节，年累积也在分钟级；深拷贝优化可能是分钟 → 秒的改进
- **低频（非热点）**：每请求 1 次调用（如单次 500KB 数据处理），实测延迟 0.0004~0.02ms，比网络往返低 3-5 个数量级；不改

### 实例

本仓 proxy 转发路径实测：
- 单次 500KB 报文深拷贝：0.0004~0.02ms（单个请求一次操作）
- 网络 RTT：~100ms（同数量级）
- 判断：非热点，深拷贝开销淹没在网络噪声中，禁优化

反例：mpsc 消息队列热路径
- 每次 try_send 可能重复若干次（背压重试、多消费者竞争）
- 单次虽只拷贝 String/Vec，但频次高，年累积显著
- 判断：热点，需 check-then-send 避免无谓克隆

### 实施检查

- [ ] 估算调用频次：每请求 N 次？循环内多少轮？
- [ ] 如 N ≤ 1，跳过深拷贝优化（除非性能量测明确指出）
- [ ] 如 N ≥ 10+ 且报文 > 100KB，才考虑优化
- [ ] 量测前后要 profile，禁凭直觉优化

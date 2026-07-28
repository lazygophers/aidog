# 04 tokenizer 常驻内存的处置方案

Type: grilling
Status: open
Blocked by: 01
Parent: [深度性能优化：全进程峰值内存 ≤200MB + 三场景 CPU 下降](../map.md)

## Question

四个 tokenizer 常驻单例各占多少内存，怎么处置？

现状（`src-tauri/crates/aidog_core/src/gateway/proxy/tokenizer.rs`）：
- `GLM4_JSON` = 19MB，`QWEN2_JSON` = 6.7MB，`include_bytes!` 编入二进制（`tokenizer.rs:19-20`）
- 两者经 `Tokenizer::from_bytes` 解析进 `OnceLock`（`tokenizer.rs:71,84`）—— HF tokenizer 解析后的 vocab / merges 结构常驻通常是 JSON 的数倍
- 另有 tiktoken `cl100k_base_singleton()`（`tokenizer.rs:60`）与 `o200k_base_singleton()`（`tokenizer.rs:65`）
- 四个都是 lazy 单例：**一旦某次请求触发，进程生命周期内永不释放**

**硬约束（来自体验红线 2）**：token 计数与费用精度不得下降。因此以下方案**已被排除**，不要在本票里再提：
- 降级为字符数 / 字节数估算
- 纯依赖上游返回的 usage 字段（上游不返或返得不全时会失真）

允许探索的方向（非穷尽）：
- 按需加载：只在真正遇到对应协议的请求时才初始化，且当前是否已经是这样
- 用完释放 / LRU：`OnceLock` 换成可驱逐的容器，闲置 N 分钟后 drop
- 子进程隔离：token 计数放独立进程，算完退出，主进程不留常驻
- 数据瘦身：`include_bytes!` 换成运行时按需读盘 / 压缩存储，或裁剪 vocab 中用不到的部分（**须先验证不影响计数结果**）
- 上游 usage 优先 + 本地兜底：上游给了就用上游的（精度不降反升），没给才本地算 —— 能降低触发概率但不能保证释放

代价评估必须覆盖：是否踩红线 1（首 token 时延，若计数在热路径上）、红线 4（冷启动，若改成启动时加载）。

## 验收

- 四个单例各自的实测常驻字节数（引 [01] 的分解数据）
- 一个选定方案 + 为什么否掉其余方案的一句话
- 该方案对四条红线的逐条影响判断

---
layer: recall
created: 1785514813
title: deterministic-pseudorandom-loadgen
category: testing
keywords: [deterministic,pseudorandom,loadgen,testing,reproducibility]
status: active
inclusion: auto
---
layer: recall
created: 1785514813

## 确定性伪随机负载生成

## 压测可复现的确定性伪随机（原子计数器+哈希）

## 问题

压测场景（尤其是性能/内存压测）需要可复现的伪随机行为，用于注入 `error_rate=0.05`（5% 请求返回 429）等。

常见做法是引 `rand` crate，但压测需要：
- **跨运行确定性**：同一压测配置多次跑结果分布相同
- **进程级独立**：无需外部 seed 文件，新进程自动产生确定的序列
- **快速求值**：无锁原子操作，不阻塞主路径

## 方案

**进程级原子计数器 + 乘法哈希** (`proxy/mock.rs:2-16`)：

```rust
static MOCK_ERROR_COUNTER: AtomicU64 = AtomicU64::new(0);

fn mock_error_hit(error_rate: f64) -> bool {
    const SCALE: u64 = 10_000;
    let threshold = (error_rate.clamp(0.0, 1.0) * SCALE as f64) as u64;
    let n = MOCK_ERROR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let scrambled = n.wrapping_mul(0x9E3779B97F4A7C15); // splitmix64 常数
    (scrambled % SCALE) < threshold
}
```

**特点**：
- 计数器 `n` 单调递增，按进程启动时刻确定初值（默认 0）
- `splitmix64` 常数 `0x9E3779B97F4A7C15` 是乘法哈希的通用打散常数，避免"每 SCALE 请求里前 N 个连续命中"的簇集
- 无依赖、无 lock contention、无堆分配
- `Ordering::Relaxed` 足够（不需跨线程 fence）

## 关键点

- **确定性**：给定 error_rate 的序列完全由进程启动顺序决定，重复压测结果稳定
- **分布均匀**：splitmix64 打散让命中在整个请求序列中均匀分布（非纯取模的前 N 个集中）
- **性能**：原子 fetch_add(Relaxed) + 乘法无分支，是最快的伪随机方案
- **升级路径**（ponytail 注释）：若需跨进程/外部 seed 控制，可换 `rand::SeedableRng`

## 用途

- mock 平台的 error_rate 注入
- 压测场景的确定性故障模拟
- 内存/CPU 基准测试（需要重复压测时结果分布一致）

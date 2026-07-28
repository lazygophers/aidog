# tokenizer 峰值与零散常驻清扫 — 详细设计

## 现状（静态盘点）

### 一、tokenizer（峰值主线）

| 位置 | 内容 |
|---|---|
| `crates/aidog_core/assets/tokenizers/glm-4.json` | **19.0MB** |
| `crates/aidog_core/assets/tokenizers/qwen2.json` | **6.7MB** |
| `gateway/proxy/tokenizer.rs:19,20` | `include_bytes!` 编入二进制 |
| `tokenizer.rs:32-56` | 前缀判定（选 encoding） |
| `tokenizer.rs:60` / `:65` | tiktoken `cl100k` / `o200k` 单例 |
| `tokenizer.rs:70,83` | `Tokenizer::from_bytes` → `OnceLock` |
| `gateway/proxy/count_tokens.rs:40` | 调用点 |
| `count_tokens.rs:67` | 用 `requested_model` 选 encoding |
| **`count_tokens.rs:74`** | **无条件先算 `est_tokens`** |
| `gateway/proxy/handler.rs:306-312` | 注释：claude-cli **每次对话前**都打 count_tokens 端点 |
| `handler.rs:311` | `is_count_tokens_endpoint` 分流 |

HF `Tokenizer::from_bytes` 解析 19MB JSON 的堆峰值实测量级 **40-120MB**，且四个单例 **lazy 但一旦触发永不释放**。

**关键性质**：`count_tokens.rs:74` 是**无条件**先算，上游成功返回时这份本地计算完全白做。最省力修法 = **只在上游失败 fallback 分支才算**。这不触碰精度（红线 2）—— 上游成功时本就用上游值。

**归因边界（诚实标注）**：[08] 那次 149MB 采样**未开 MITM、无 glm/qwen 请求**（mock 协议不走 HF tokenizer 分支）⇒ tokenizer 与 MITM **不在那 149MB 里**。故本 task 的 tokenizer 项是**峰值口径**风险（200MB 目标是峰值口径），不是稳态 149MB 的组成部分。

### 二、presets 重复解析

| 位置 | 内容 |
|---|---|
| `gateway/peak_hours.rs:51` | `OnceLock` 解析 bundled presets |
| `gateway/defaults_sync.rs:36` | 第二份独立解析 |

同一份 ~105K 源文本解析两遍，两份 `serde_json::Value` 常驻。修法：抽单一 `OnceLock<Value>`，两处复用。

### 三、死代码

`gateway/coding_plan.rs:15` / `:23` —— `#[allow(dead_code)]`，注释自承当前无 Rust 路由消费。生产零调用点 → **整删**。

⚠️ 删前 grep 全 workspace 确认无调用（memory `cargo-workspace-gate-not-single-crate`：改公开签名必跑 `--workspace`，`-p aidog_core` 会漏 `commands_*` 调用点）。

### 四、无界容器

| 位置 | 内容 |
|---|---|
| `mitm/cert_signer.rs:66` + `:82-97` | 证书缓存**无上界** |
| `mitm/mod.rs:57` | `suspects` 表，`SUSPECT_TTL_SECS=600` 但**无 sweep** |

全仓仅有的两个真无界容器，只在开 MITM 后增长。修法 idiom 直接抄 `proxy/devin.rs:50`：`len() > N → retain`（已有成例，不新造机制）。

cert 缓存 evict 后该 host 首次连接需重签 —— 只影响新连接，**不影响已建立连接**。

### 五、`AGG_DEDUP_CAP`

`proxy/mod.rs:167` `AGG_DEDUP_CAP = 8192`，`:187` FIFO 淘汰。8192 是拍脑袋值，对单用户应用偏大。降到合理值（先量单条字节与实际去重窗口需求再定）。

## 方案（当前方案 = 精简守现状）

改动面五处，彼此独立，可并行：

1. `count_tokens.rs:74` → 惰性化：只在上游失败 fallback 分支算
2. presets 双解析 → 单一 `OnceLock`
3. `coding_plan.rs` 死代码整删
4. `cert_signer` + `suspects` 加界（抄 `devin.rs:50` idiom）
5. `AGG_DEDUP_CAP` 降值

**不动**：`tokenizer.rs` 的 `pick_encoding` 选型逻辑与四个单例本身（改的是**何时触发**，不是**怎么算**）。

## 为什么不选别的

| 备选 | 否决理由 |
|---|---|
| 降级为估算 / 纯依赖上游 usage | **红线 2 明令排除**（token 计数与费用精度不得下降） |
| 用完 `drop` 释放 tokenizer | `OnceLock` 语义即永不释放；改成可释放需引入锁 + 重复解析成本，且 claude-cli 高频打该端点 → 反复重解析更糟 |
| tokenizer 移到子进程隔离 | 收益 = 把 40-120MB 挪到另一个进程，**全进程总和口径下零收益** |
| 启动时预热 tokenizer | **红线 4 明令排除**（冷启动不得变慢） |
| 把 19MB JSON 改外部文件按需读盘 | `include_bytes!` 的是**二进制体积**，非常驻堆；堆峰值来自 `from_bytes` 解析，改读盘不解决 |

## 数据流（验证链路）

```
mock 平台模拟「上游 count_tokens 成功」与「上游失败」两条路径
  → 成功路径：footprint 采 phys_footprint，确认无 HF tokenizer 初始化跃升
  → 失败路径：逐条比对 fallback token 数与改动前一致
  → presets 解析次数：trace 或计数器证明为 1
  → cargo clippy --workspace + cargo test --workspace
```

## 可能性分支（不进当前方案，仅留痕）

- **tokenizer 空闲超时释放** — 触发条件：若惰性化后仍有用户长期使用 glm/qwen 且峰值超预算。需把 `OnceLock` 换成带 TTL 的结构 + 重解析成本评估。
- **精简 glm-4.json / qwen2.json 词表** — 触发条件：若解析峰值仍是主要瓶颈。风险：动词表直接压红线 2（精度），需逐 token 等价性验证，代价极高。
- **MITM 证书缓存持久化到磁盘** — 触发条件：若加界后重签频率影响 MITM 连接建立延迟。

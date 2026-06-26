# 调度同优先级按最早过期排序扩展至全部模式

## 背景 / 动机

当前「同优先级按最早过期时间排序」(`expiry_sort_key`，快过期平台先用以省额度) **仅在 `Failover` 路由模式生效** (`candidates.rs:155-168` 三键排序 `(Reverse(level_priority), priority, expiry_sort_key)`)。其余四模式 (`LoadBalance` / `HealthAware` / `LeastLatency` / `Sticky`) 不感知 `expires_at`，导致快过期平台额度可能浪费。

用户决策 (2026-06-26)：**扩展到全部模式**。已知语义权衡 (cortex `group-scheduling-breaker`)：加权随机模式硬插 expiry 会弱化负载均衡语义；用户已接受，按「同档位内 tiebreak」方式注入，不破坏各模式主排序键。

## 目标

在不改变各路由模式**主排序键**的前提下，把 `expiry_sort_key(gp.platform.expires_at)` 作为**同档位 tiebreak** 注入全部模式：

- **LeastLatency** (`ordering.rs:73-83 order_least_latency`)：主键延迟 EMA 升序不变；同 EMA 档内插入 expiry 升序，**置于现有 `level_priority` tiebreak 之前**（expiry 是更强的"用掉它"信号）。
- **LoadBalance / HealthAware** (`ordering.rs:45-69 order_load_balance`)：加权随机选首不变；基础降序排序 `sort_by_key(Reverse(effective_weight))` 改为同权重档内按 expiry 升序 tiebreak（影响重试顺序，不影响首选随机性）。
- **Sticky**：复用 `order_load_balance`，随 LoadBalance 改动自动覆盖；绑定逻辑 (`apply_sticky`) 不变。

## 约束

- 复用既有 `expiry_sort_key()` (`ordering.rs:22-28`)，禁新写过期判定逻辑。
- `expires_at` 字段在 `gp.platform.expires_at: i64`，已过期候选由 `candidate_state` 上游过滤，此处只对未过期候选排序。
- `apply_coding_plan_priority` 调用顺序不变（仍在各 mode 排序之后、`apply_sticky` 之前）。
- 纯 Rust 后端改动，零前端 / TS 触碰。
- `cargo clippy` warning 必须清（项目硬规）。

## 产出

1. `src-tauri/src/gateway/router/ordering.rs`：`order_least_latency` 与 `order_load_balance` 注入 expiry tiebreak（`order_least_latency` 的 `then_with` 链插 expiry；`order_load_balance` 基础排序由 `sort_by_key` 改 `sort_by` 加 expiry 次键）。注释说明 tiebreak 语义与位置。
2. 测试 `src-tauri/src/gateway/router/test_ordering.rs`：每模式各加同档位 expiry tiebreak 用例（LeastLatency 同 EMA 不同 expiry；LoadBalance 同权重不同 expiry 的重试顺序；Sticky 经 LoadBalance）。
3. 测试 `src-tauri/src/gateway/router/test_candidates.rs`：非 Failover 模式 expiry 集成用例（对照现有 Failover expiry 集成测试 479-560 行模式）。

## 验证

- `cd src-tauri && cargo test` 全绿（含新增用例 + 既有 Failover expiry 用例不回归）。
- `cd src-tauri && cargo clippy` 零 warning。
- 人工核对：LeastLatency 同延迟档、LoadBalance 同权重档下，`expires_at` 小者排前；永不过期 (`expires_at=0`) 排末尾。

## 失败处理

- 若加权随机 + expiry tiebreak 在测试中表现出首选随机性被破坏 → 确认 expiry 仅作用于"基础降序"而非"随机选首"，必要时调整注入点（随机 pick 仍基于 weight，不基于 expiry）。
- 测试断言不稳定（随机种子）→ 用固定 seed 构造确定性输入。

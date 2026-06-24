# PRD — coding plan 平台优先调度

## 背景

`platform.coding_plan: bool`（`models/platform.rs:141`）标记平台是否为订阅制 coding plan。
当前候选排序 `router/candidates.rs` + `router/ordering.rs` **完全未把 coding_plan 当排序因子**，
排序键仅为 level_priority / weight / latency / sticky。

订阅制 coding plan 额度按月包干，按量计费（balance）按 token 扣费。
同一分组内若混有 coding plan 与非 coding plan 平台，当前调度不会偏向前者，导致本可消耗的订阅额度被闲置、反而走了更贵的按量平台。

## 需求（用户原话）

> 对于不确认是否的 coding plan 的，优先使用 coding plan 的平台。

### 解释定调（brainstorm 被用户跳过，用户指示「继续」，故按最合理解释推进，假设在此备查）

调度在无明确依据偏向某平台时，**优先消耗 coding plan（订阅制）额度**，省钱。
落地为：分组内候选排序新增 coding plan 偏好 —— `coding_plan=true` 平台排在非 coding plan 之前。

## 范围（单一交付）

仅改后端候选排序逻辑，无前端 / DB schema 变动。

### MUST

1. 候选排序（`router/candidates.rs` 第 2 步「按路由模式排序」之后、第 3 步合并之前，或等价位置）
   新增 **coding plan 优先**：`coding_plan=true` 平台整体排在 `coding_plan=false` 之前。
2. coding plan 偏好为 **主排序键**；原有 per-mode 排序（Failover priority / LoadBalance 加权随机 / LeastLatency 延迟 / Sticky）
   在每个 coding-plan bucket **内部**保持原语义不变（即先按 mode 排好，再按 coding_plan 做稳定分桶上浮）。
3. 覆盖 **所有路由模式**（Failover / LoadBalance / HealthAware / LeastLatency / Sticky）。
4. active 桶与 probe 桶各自独立应用该偏好（probe 仍整体在 active 之后，不因 coding_plan 跨桶上浮）。
5. 显式 model_mapping 命中的目标平台仍最高优先（第 4 步行为不变，coding plan 偏好不得覆盖显式映射）。
6. 单平台分组分支（candidates.rs:72）不受影响（无可排序对象）。

### MUST NOT

- 不改 `coding_plan` 字段语义 / 类型。
- 不引入前端开关 / 配置项（保持最小面；若后续需可配再开 task）。
- 不破坏熔断 / auto_disabled 过滤逻辑（仅在已过滤的 active/probe 集合上调整顺序）。
- 不动 sticky 绑定写入语义（sticky 命中平台仍提首位；coding plan 偏好在 sticky 之前应用，sticky swap 为最终步，优先级最高 —— 与显式映射类似）。

## 验收标准

- `cd src-tauri && cargo build` 通过，`cargo clippy` 零 warning。
- `cargo test` 全绿；新增单测覆盖：
  - 混合分组 Failover：coding plan 平台排在非 coding plan 之前，同 bucket 内仍按 priority。
  - LoadBalance：coding plan bucket 整体在前，bucket 内加权随机不变。
  - probe 平台不因 coding_plan 跨到 active 前。
  - 显式 model_mapping 目标平台仍居首（即便它非 coding plan）。
- 排序为稳定操作，不破坏既有 test_candidates / test_ordering 断言（若断言因新偏好需调整，须在 coding_plan 维度上对齐而非削弱）。

## 风险 / 备注

- 实现优先用稳定排序 `sort_by_key(|gp| !gp.platform.coding_plan)`（false 在前即 coding 在前），
  避免破坏已排好的 mode 内顺序（Rust `sort_by_key` 为稳定排序）。
- sticky 模式：coding plan 偏好须在 `apply_sticky` **之前**应用，否则 sticky swap 后又被 coding 分桶打乱。

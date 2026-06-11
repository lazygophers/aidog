# ST3: coding plan 预估（Kimi 精确 + 方案 B 拟合）

- **目标**: coding plan utilization 增量预估
- **产出**:
  - quota.rs: Kimi coding plan 真查(:265)**保留 limit/remaining**（当前算完 utilization 丢弃）→ 存入 est_coding_plan 基数
  - db.rs `estimate_coding_plan(db, platform_id, token)`：读 est_coding_plan JSON → 更新：
    - Kimi(has_base): est_utilization += token×(100/limit)
    - GLM/MiniMax(方案B): tokens_since_real+=token; 有 coef → est_utilization=util_at_last_real+tokens_since_real×coef; 冷启动(无 coef)不预估显真值
    - read-modify-write 平台级串行（单事务/CAS 避并发覆盖）
  - est_coding_plan JSON 结构见 design（tiers[].est_utilization/coef_per_token/util_at_last_real/tokens_since_real/has_base）
- **验证**: cargo build + 单测（Kimi 精确增量 / 方案B 拟合 coef / 冷启动不预估 / reset 丢样本）
- **资源**: research/coding-plan-base-feasibility.md、design.md 方案B 算法
- **依赖**: ST1
- **失败处理**: 并发覆盖 → 平台锁/事务；reset 检测 util_real<util_at_last_real

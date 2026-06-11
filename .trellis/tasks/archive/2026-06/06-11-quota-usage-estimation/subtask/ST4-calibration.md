# ST4: 真查校准（降频核心）

- **目标**: 阈值触发真实 query 覆盖预估
- **产出**:
  - db.rs/proxy.rs 后台预估时检查 `now - last_real_query_at > 300_000 || estimate_count >= 100`
  - 满足 → 真实 `query_quota`(quota.rs:407, **async 锁外调用**) → 覆盖 est_balance_remaining + est_coding_plan（Kimi 存 limit；方案B 拟合 coef = (util_real-util_at_last_real)/tokens_since_real，无跨 reset 时）+ last_real_query_at=now + estimate_count=0 + tokens_since_real=0
  - 拟合系数更新逻辑（GLM/MiniMax）：真查覆盖时若 tokens_since_real>0 且无 reset → 更新 coef
- **验证**: cargo build + 单测（阈值触发、覆盖重置、coef 拟合）
- **资源**: research/calibration-trigger.md、design.md
- **依赖**: ST2, ST3
- **失败处理**: 持锁跨 await → query_quota 锁外，结果回写单 SQL

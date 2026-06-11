# ST2: 预估偏差 + 校准对齐真实

- **目标**: 预估偏差小 + 真查校准时 est 严格对齐真实
- **产出** (estimate.rs/db.rs/lib.rs):
  - 调查 est 与真实差距根因（读 run_calibration/write_real_quota/apply_balance_delta/apply_coding_plan_delta）
  - **校准对齐**：write_real_quota 严格覆盖 est_balance_remaining=真实 balance + est_coding_plan tiers est_utilization=真实 util（util_at_last_real=真实, tokens_since_real=0, 拟合 coef 更新但当前值=真实）；确认无漏字段/无旧 est 残留
  - **冷启动初始化**：last_real_query_at==0 / est 未初始化 → tray 读取或启动触发真查初始化 est=真实（避免显 0/旧偏差）
  - 测试：write_real_quota 后 est==真实；冷启动初始化
- **验证**: cargo test 0；校准对齐 + 冷启动
- **资源**: design.md、estimate.rs(校准/拟合)、db.rs(quota)、wiki aidog-quota-estimation-architecture
- **依赖**: 无
- **失败处理**: 偏差根因不明 → 加日志/对比 est vs 真查；卡 3 次停报告

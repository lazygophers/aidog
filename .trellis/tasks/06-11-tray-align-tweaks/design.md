# Design: tray 对齐/格式/预估对齐

## 1+2 渲染（lib.rs）
- coding value：`format!("剩 {:.0}%", 100-util)` → `format!("{:.0}%", (100.0-util).max(0.0))`（删"剩"）
- 第二行右对齐：两行模式 NSTextTab —— 第一行标签列用 left tab；第二行值列用 **RightTabStopType**（值右对齐到该列右边界 location）。即每列两个对齐语义：标签 left @列起，值 right @列末。实现：值行 tab stop 用 `NSTextTab(RightTabStopType, 列右边界 location)`；或值行单独 paragraph right tabs。简化：值列 tab location = 列右边界（列起+列宽），right 对齐

## 3+4 预估偏差 + 校准对齐（estimate.rs/db.rs/lib.rs）
- 调查 est 偏差根因（读 estimate.rs run_calibration/write_real_quota/apply_*）
- **校准对齐（第4点核心）**：真查校准时 `write_real_quota` 必须**严格覆盖** est：
  - est_balance_remaining = 真实 balance.remaining
  - est_coding_plan tiers est_utilization = 真实 util（util_at_last_real=真实, tokens_since_real=0）
  - 拟合 coef 更新但 est 当前值 = 真实（不被旧 est 残留）
  - 确认 write_real_quota 没漏字段 / 没被后续 est 覆盖
- **冷启动初始化（第3点）**：est 从未校准（last_real_query_at=0）或 est=0 时，tray 读取/启动触发一次真查初始化 est=真实（避免冷启动显 0/旧偏差）。可在 refresh_tray_menu 或启动时：若 tray 平台 last_real_query_at==0 → 真查初始化
- 确保预估偏差小：校准对齐 + 冷启动初始化双管

## 验证
- cargo test + tsc；第二行右对齐 + coding 无"剩"；校准后 est==真实（测试 write_real_quota 覆盖）；冷启动初始化；GUI 用户验

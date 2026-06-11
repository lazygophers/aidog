# PRD: tray 显示细节调整（对齐/格式/预估偏差）

## 需求
1. **第二行右对齐**：列内第二行(值)相对第一行(标签)右对齐（标签左、值右对齐到列宽）
2. **删"剩"字**：coding 直接展示数字 `{x}%`（去掉 `剩 ` 前缀，lib.rs:1311）
3. **预估偏差修**：tray 显 est 预估值与平台真实差距大 → 保留预估(降频)但偏差不该那么大，修根因
4. **真实变化即对齐**：真查校准发生时，est 必须严格对齐(重置=)真实值（覆盖 est + 重置累积/拟合基线），真实变化预估立即同步

## 现状
- lib.rs:1311 coding `format!("剩 {:.0}%", 100-util)`
- 两行渲染 NSTextTab 列左对齐（第一行标签/第二行值，均 left）
- est_balance_remaining/est_coding_plan 预估 + 5min/100次校准；冷启动/拟合

## 改动
### 1+2 渲染（lib.rs）
- coding value 删"剩"：`format!("{:.0}%", (100-util).max(0))`
- 第二行右对齐：值列用 NSTextTab RightTabStopType（值右对齐到列右边界 = 列宽位置），第一行标签保持 left；或列内 value 右对齐 name 宽度
### 3 预估偏差（estimate.rs/db.rs/lib.rs，调查 + 修）
- 调查 est 与真实差距大根因：① 冷启动 est 未初始化（last_real_query_at=0 / est=0 显偏差）② 校准滞后(5min/100次期间累积偏差) ③ 拟合 coef 偏差 ④ balance cost 累积误差
- 修方向：tray 平台 est 冷启动/未校准时触发真查初始化（避免显 0/旧）；或校准阈值更合理（更频繁/首次启动真查 tray 平台）；确保 est 接近真实

## 验收
- 第二行右对齐；coding 无"剩"纯数字
- tray est 与真实差距小（冷启动初始化 + 及时校准）
- cargo test + tsc；GUI 用户验对齐 + 数值准

## Subtask
- ST1: 渲染微调（删"剩" + 第二行右对齐）
- ST2: 预估偏差调查 + 修（冷启动初始化/校准）

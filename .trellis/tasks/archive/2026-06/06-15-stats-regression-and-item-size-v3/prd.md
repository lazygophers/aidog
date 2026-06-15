# Stats 回归修复 + 模型筛选 item 尺寸 v3

## A. 回归（P0）—— 整个使用统计无数据
根因：commit 6c358f2 给 dimension platform 加 `LEFT JOIN platform p` 后，SQL `WHERE deleted_at = 0` 触发 `ambiguous column name: deleted_at`（proxy_log 与 platform 两表皆有 deleted_at 列）→ query_stats_inner 返 Err → 前端无数据。
- 测试复现：`cargo test query_stats_platform_dim_and_filter`（已加）报 ambiguous column。
- 修复：where_parts 全列加 `proxy_log.` 前缀；EFF_PID 内 `platform_id` → `proxy_log.platform_id`；dimension SQL 硬编码 `deleted_at` → `proxy_log.deleted_at`。
- 验证：测试过，291 passed。

## B. 模型筛选 item 仍过小（v2 力度仍不足）
v2 (bf6796c): fontSize 14 / padding 10px12px / width 320。用户反馈「字都看不到」。
根因推测：button 元素默认不继承父字号/字体，内联 14px 在 Tauri webview 视觉偏小。
方案 v3（FilterOption）：
- fontSize 14 → **15**
- padding → **"11px 14px"**
- lineHeight 1.4 → **1.5**
- 加 `fontFamily: "inherit"`（button 默认不继承）
- 搜索 input fontSize 13 → 14
- dropdown width 320 保持（长模型名够，靠滚动+搜索）

## 验证
- cargo test + clippy + tsc 全过。
- 视觉：item 字号 15、行距宽松、可读。

## 非目标
- 不改 dropdown 宽度逻辑。

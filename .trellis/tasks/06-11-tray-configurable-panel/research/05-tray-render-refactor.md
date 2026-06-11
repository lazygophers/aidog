# Research: tray 渲染重构（单平台 → 多 item）

- **Query**: lib.rs 从 get_tray_platform 单平台 → 读 settings tray config → 渲染多 item attributedString
- **Scope**: 内部 Rust
- **Date**: 2026-06-11

## 现状渲染链路（lib.rs）

1. `tray_quota_text(app) -> Option<String>`（lib.rs:1247）：读**单**平台 `db::get_tray_platform`（:1249），算 name + 第二行（coding 剩余% 或 balance），拼 `"{name}\n{second}"`。
2. `build_tray_menu`（:1274）：菜单内嵌一行 quota item（:1295）。
3. `set_tray_attributed_title`（:1322）：单段 attributedTitle（NSFont 9pt + 居中段落 + baseline）。
4. `refresh_tray_menu`（:1385）：有 quota text → 隐 logo + set_title 兜底 + attributedTitle 覆盖；无 → 恢复 logo + 清 title。
5. 触发点：`platform_set_tray`(:207)、`proxy_start/stop`(:332/357)、后台 `tray-refresh` 事件(:1488)。

## 重构方案

### 数据层
- 新增 `tray_config(app) -> TrayConfig`：读 `get_setting("tray","config")`（gateway/db.rs:560），解析 typed struct（参考 ProxyLogSettings 模式 lib.rs:873）。空/失败 → 默认空配置。
- 每个 platform item 取值：复用现 `tray_quota_text` 内逻辑（EstCodingPlan::from_json、est_balance_remaining），但需**按 platform_id 查指定平台**（现 get_tray_platform 是 `WHERE show_in_tray=1`，需改为 `get_platform(id)` 已存在的取单平台函数）。
- today_usage item：见下 05b。

### 渲染层（替换 set_tray_attributed_title 单段逻辑）
改用 `NSMutableAttributedString`（API 已核实可用，见 01）：
```
let mut_str = NSMutableAttributedString::new();
for (idx, seg) in segments.iter().enumerate() {
    if idx>0 { append(separator_attr_str) }   // single_line 分隔符
    let attrs = {NSFont(seg.font_size), NSForegroundColor(seg.color), 段落样式};
    let part = NSAttributedString::initWithString_attributes(seg.text, attrs);
    mut_str.appendAttributedString(&part);
}
button.setAttributedTitle(&mut_str);
```
- layout=two_line：段间用 `\n` 而非 separator（沿用现固定行高段落样式 lib.rs:1348-1353）。
- layout=single_line：段间 separator，单段落。

### 需新增依赖 feature
- `Cargo.toml:42` objc2-app-kit features 加 `"NSColor"`（现未含，见 01）。

### 触发点调整
- `platform_set_tray` 命令 → 废弃/改造（见 06 迁移）。新增 `tray_config_set` 命令（或直接复用通用 `settings_set` + 前端调用后再 invoke 一个 `tray_refresh` 命令）。
- 现有 `tray-refresh` 事件机制（lib.rs:1488）、proxy_start/stop 的 refresh_tray_menu 保留。
- build_tray_menu 的下拉 quota item（:1295）：可改为多行（每平台一 menu item）或保留概要。

## 05b. 今日消耗数据源

proxy_log **无 cost 列**（`PROXY_LOG_COLUMNS` gateway/db.rs:617 仅 input/output/cache_tokens + created_at ms）。cost 在查询时算（`resolve_price` db.rs:1156，按 model+platform_type 取价 × tokens）。

today 聚合方案：
- 时间过滤：`created_at >= 今日0点ms`（created_at 是毫秒 epoch，参考 stats `created_at/1000,'unixepoch'` db.rs:989；今日0点需按本地时区算）。
- **metric=tokens**（简单）：`SUM(input_tokens+output_tokens)` WHERE created_at>=今日0点 AND deleted_at=0。参考 usage_stats（db.rs:841）SQL 模式，加时间 where。
- **metric=cost**（复杂）：需按 (model, platform_type) 分组 tokens → 逐组 resolve_price × tokens 累加。无现成"今日总 cost"函数，需新增 db 函数（GROUP BY model,target_protocol 拿 tokens，循环 resolve_price）。stats.rs / db stats 有按维度聚合 tokens 先例（db.rs:1014 dimension breakdown）可参考。

→ **建议**：MVP 今日消耗先做 **tokens**（直接 SUM，无定价依赖），cost 作为进阶（需新 db 聚合 + resolve_price 循环）。需 design/用户确认优先级。

## 与 a391ad4d 协调

a391ad4d 正改 lib.rs tray 区（同 set_tray_attributed_title / refresh_tray_menu / tray_quota_text 区域）。重构需：
- 基于 a391ad4d 落地后的最新 tray 渲染函数签名再动手（避免冲突）。
- 本文件基于当前 HEAD（3c3ef6e）可见状态，函数行号可能因 a391ad4d 改动偏移。

## Caveats

- 今日0点时区：created_at 毫秒 epoch，"今日"按本地时区还是 UTC 需确认（stats 用 unixepoch 默认 UTC，db.rs:989）。可能与用户直觉的"今天"差时区。标注需 design 决策。
- get_platform(id) 单平台取函数需确认存在（应有，platform_get 命令 lib.rs 周边）。

# 保留时间单位选择器 — PRD (主入口)

## 目标
ProxyLogSettings 三个 retention 字段（user_request_retention_days / upstream_request_retention_days / retention_days）从「固定按天」改为「值 + 单位（小时/天/周）」。单位可在 UI 下拉选择。

动机（用户确认）:
- [x] 单位选择器覆盖**三项全部**（user/upstream/整体日志保留）
- [x] **三项默认全改 6h**（新装默认；原始 body 6h 够排查；整体日志 6h 激进但用户确认）
- [x] 已有 settings.json **保留原值**，缺 unit 字段 serde default = "day"（零迁移、零行为变化）

## 边界
范围内:
- [ ] Rust `ProxyLogSettings` 加 3 个 `*_retention_unit: RetentionUnit` 字段（serde lowercase, default "day"）
- [ ] `RetentionUnit` enum { Hour, Day, Week }（serde rename_all lowercase）
- [ ] `retention_cutoff` 改签名接 secs（或加 unit helper），3 处 cleanup_* caller 传 secs
- [ ] TS `ProxyLogSettings` 类型 + useSystemSettings state/setters + updateLogSettings payload
- [ ] UI LogSettingsSection 三项加 `<select>` 单位下拉（挨着数字 input）
- [ ] 8 locale 文件加单位 label key（hour/day/week）+ label 文案「保留天数」→「保留时间」
- [ ] 新装默认：unit=hour value=6（三项）；老配置缺 unit → default day 保持 7/7/90

范围外:
- [ ] stats_agg retention（Settings.retention_days 365，独立 struct，UI 不显）— 不动
- [ ] app_log retention_hours（已按小时，独立）— 不动
- [ ] inbox_retention_days（通知，独立）— 不动
- [ ] DB schema migration（settings 是 serde JSON，新字段 serde default 自动填，无 migration）

## 关键约束
- 字段名保留 `*_retention_days`（向后兼容 serde，老 key 不 rename；语义从「天数」变「数值」，unit 决定单位）
- cleanup_* 现有调用：`cleanup_user_request_fields(db, settings.user_request_retention_days)` → 改 `cleanup_user_request_fields(db, value, unit)` 或 caller 算 secs 传入
- is_memory 短路（[[dual-db-aggregate-is-memory-shortcut]]）：cleanup_* 内部 is_memory 分支不变

## 验收标准
- [ ] Rust 编译 + clippy 0 新增 + cargo test 全过
- [ ] yarn build + check-i18n 零缺失
- [ ] 老配置（无 unit 字段）加载后行为不变（7d/7d/90d）
- [ ] 新装默认 6h（三项）
- [ ] UI 三项各带单位下拉，切单位即时 updateLogSettings
- [ ] cleanup_* 按单位正确换算 secs（hour×3600, day×86400, week×604800）

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 任务/子任务: task.json (`skein subtask list retention-unit-selector`)

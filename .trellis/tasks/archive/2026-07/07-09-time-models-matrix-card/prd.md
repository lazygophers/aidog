# 时段模型配置与模型配置合并为矩阵 card

## Goal

把 ModelsSection（默认模型档，单列 5 槽）与 TimeModelsSection（按时段切换模型档，独立 section 每 rule 一 card）合并成**单个矩阵 card**：列 = 配置档（默认列 + 各时段档），行 = 模型槽位（default/sonnet/opus/haiku/gpt），每格 = 该档该槽的模型值。时段档列头点弹窗编辑 windows。

用户 UI 草图：
```
          默认  高峰  1:01~2:01(周日)  00:00-23:59(每月1日)
opus       .     .     .                .
sonnet     .     .     .                .
```

## Requirements

- 合并 ModelsSection + TimeModelsSection 为单矩阵 FormSection
- 列 = [默认列（platform.models）] + [时段档列（time_models.rules[*]）]
- 行 = 5 槽（default/sonnet/opus/haiku/gpt），每格 input
- 默认列保留下拉（pinyinMatch）+ fillAll + fetchModels（行为不退化）
- 时段档列复用同源下拉（availableModels || defaultList）
- 列头：默认列固定「默认」；时段档列显示 windows 紧凑描述，点击弹窗编辑 windows
- PeakWindow schema 扩展：start_minute/end_minute（0-59 Option，缺=0）+ days_of_month（1-31 Option）
- 互斥约束：单窗口 days_of_week 与 days_of_month 互斥（UI radio：无 / 周几 / 每月几日）
- 矩阵横向滚动（overflow-x: auto，列宽固定 ~160px）

## Acceptance Criteria

- [ ] 矩阵 card 渲染：列=档，行=5 槽，横向滚动
- [ ] 默认列行为不退化（下拉/fillAll/fetchModels）
- [ ] 时段档列 windows 弹窗编辑（minute + 维度 radio 互斥 + day_of_month）
- [ ] 列增删改顺序可用（从高峰时段导入 / 添加列 / 上下移 / 删除）
- [ ] Rust PeakWindow 扩展 + hit 支持 minute + day_of_month，向后兼容旧数据
- [ ] cargo test（peak_hours + time_models）+ cargo clippy 0 + yarn build + check-i18n 通过

## Decision (ADR-lite)

**Context**: 用户要矩阵 UI 合并模型档与时段档，草图含 minute 精度 + day_of_month 维度，超出现有 hour+weekday schema。

**Decision**:
1. schema 扩展 minute + monthly（PeakWindow 加 start_minute/end_minute/days_of_month Option）
2. 列头点弹窗编辑 windows（矩阵主体只管模型值，windows 编辑跳层）
3. 单窗口 days_of_week 与 days_of_month 互斥（UI radio 单选，语义清晰）

**Consequences**:
- 跨 Rust+TS+preset+UI 四层改动，工作量大但 schema 一次性补齐
- 向后兼容：旧 peak_hours 数据无新字段 → start_minute/end_minute absent=0, days_of_month absent=不过滤（serde Option 默认）
- 互斥简化 hit 逻辑（无需 AND/OR 组合判定）
- preset glm peak_hours（刚配的 hour 精度）无需回填，向后兼容

## Out of Scope

- year 级重复（每年几月几日）— 用户未提
- 列拖拽排序（用上下移按钮）
- 矩阵每格独立下拉源（统一复用 availableModels || defaultList）

## Technical Notes

- ModelsSection 现状：`src/pages/platforms/formSectionsEndpoints.tsx:137`，数据 `models: Record<ModelSlot, string>`（platform.models）
- TimeModelsSection 现状：`src/pages/platforms/formSections.tsx:602`，数据 `rules: TimeModelRule[]`（platform.extra.time_models）
- 合并仅 UI 层，数据通道分离：默认列→platform.models，时段档列→time_models[idx].models[slot]
- PeakWindow Rust struct：`src-tauri/src/gateway/peak_hours.rs:13`
- hit fn：`peak_hours.rs:50`（hour+weekday），扩展加 minute 比较（绝对分钟 start*60+minute 半开 [start,end)）+ day_of_month 过滤
- utc_hour_weekday：`peak_hours.rs:37`，扩展返 minute + day_of_month（或新 fn utc_time）
- time_models::resolve_time_models：`time_models.rs`，复用 hit，自动支持新字段
- 前端 PeakWindow：`src/domains/platforms/defaults.ts:10` + `src/services/api/types/part1.ts`
- 前端 isCurrentlyPeak：`src/utils/peakHours.ts`（与 Rust hit 对称，需同步扩展）
- PeakWindow 使用面：Rust 4 文件（peak_hours/time_models/stats_today/db）+ TS 7 文件

## hit 语义（implement 细则）

- minute：窗口边界用绝对分钟 start_min = start_hour*60 + start_minute, end_min = end_hour*60 + end_minute；半开 [start_min, end_min)；跨天 end_min <= start_min 按 `t >= start_min || t < end_min`
- 当前时刻 t = hour*60 + minute
- day_of_month：1-31；当前日不在列表 → 不命中；不存在日期（2月30）自然不命中
- 互斥：UI 保证单窗口 days_of_week 与 days_of_month 不同时 Some；hit 层若同时 Some 取 AND（防御性，正常不触发）
- 向后兼容：旧数据无 start_minute/end_minute → 默认 0；无 days_of_month → 不过滤

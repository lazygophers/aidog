# 实施计划 — 时段模型配置与模型配置合并为矩阵 card

PRD: `prd.md`

## ST1: Rust PeakWindow 扩展 + hit + 测试

- `peak_hours.rs:13` PeakWindow 加 `start_minute: Option<i32>` / `end_minute: Option<i32>` / `days_of_month: Option<Vec<i32>>`（serde default，向后兼容）
- `utc_hour_weekday` → 扩展或新 `utc_time` 返 (hour, minute, weekday, day_of_month)
- `hit` 扩展：minute 绝对分钟半开 + day_of_month 过滤 + days_of_week + days_of_month 同 Some 时 AND（防御）
- `resolve_multiplier` / `is_in_peak_window` / time_models::resolve 自动受益
- 测试：minute 边界 / 跨天+minute / day_of_month 命中与不存在日期 / 向后兼容旧数据（无新字段）

## ST2: TS PeakWindow 扩展 + 类型同步

- `src/domains/platforms/defaults.ts:10` PeakWindow 加 start_minute?/end_minute?/days_of_month?
- `src/services/api/types/part1.ts` 同步（单一真值，import type 复用）
- `src/utils/peakHours.ts` isCurrentlyPeak / hit 对称扩展（与 Rust 同逻辑）

## ST3: 矩阵 card UI

- 新组件 `ModelsMatrixSection`（formSectionsEndpoints.tsx 或新文件），合并 ModelsSection + TimeModelsSection
- props: models + handleModelChange/Select + activeDropdown + availableModels + defaultList + rules + setRules + peakHours + fillAll/fetchModels
- 布局：行=5 槽 label，列=[默认] + rules.map；横向 overflow-x auto；列宽 ~160px
- 默认列：保留下拉 + fillAll/fetchModels action（section 级）
- 时段档列：每格 input + 复用下拉源（availableModels || defaultList）
- 列头：默认列「默认」；时段档列 windows 紧凑描述（"06-10" / "06-10(周一)" / "全天(每月1日)"）+ 点击开弹窗 + 列操作（上下移/删除）
- section action：「从高峰时段导入」+「添加时段档」

## ST4: 列头 windows 弹窗编辑器

- createPortal(document.body)（modal-window-center-rule）
- 字段：起 hour:minute + 止 hour:minute + 维度 radio（无/周几/每月几日）+ 周几 button 组（维度=周几时）+ day_of_month 多选（维度=每月几日时）+ 多窗口列表（add/remove）
- 维度 radio 切换时清空另一维度字段（互斥）
- 保存→updateRule(idx, { windows })

## ST5: 数据通道 + 挂载

- `usePlatformForm.ts` / `usePlatformsState.ts`：models（默认列）+ time_models（时段档列）state 已存在，矩阵直连
- `PlatformEditForm.tsx`：替换 ModelsSection + TimeModelsSection 两处为单个 `<ModelsMatrixSection />`
- 删除独立 TimeModelsSection 挂载（formSections.tsx:602 组件可保留导出但不再用于此页，或删）

## ST6: i18n 8 语言

- 新 key：矩阵列头 / 弹窗标题 / 维度 radio 标签 / day_of_month / 紧凑描述格式
- 8 locale 全覆盖（zh-Hans/en-US/ar-SA/fr-FR/de-DE/ru-RU/ja-JP/es-ES）
- check-i18n 0 缺失

## ST7: 验证

- cargo test（peak_hours + time_models 全套）+ cargo clippy 0 warning
- yarn build（tsc + vite）
- node scripts/check-i18n.mjs 0 缺失
- 手动验证：旧平台数据（无新字段）加载不炸 + 新矩阵编辑 + 时段切换命中

## 依赖

- ST1 → ST2（类型对齐）→ ST3/ST4（UI）→ ST5（挂载）→ ST6（i18n）→ ST7（验证）
- ST3 与 ST4 可并行（矩阵主体 vs 弹窗）

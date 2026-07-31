# 分时段模型时区支持 — PRD (主入口)

## 目标

用户原话：「模型配置 的分时段的模型配置的时间段的选择不支持时区（默认应该以用户时区展示，存储是 utc-0 的时区，允许切换）」。

`time_models`（分时段模型配置）的时段编辑器 `WindowsEditModal` 是**裸 UTC 数字直写直读**
（`src/pages/platforms/WindowsEditModal.tsx:140-160`），界面无任何时区提示 —— 用户填「14」实际生效
UTC 14:00 = 北京 22:00。而同结构的 `peak_hours` 编辑器早有本地/UTC 双向换算
（`src/pages/platforms/formSections.tsx:453-473`）。存储两侧都已是 UTC+0
（`gateway/time_models.rs:23` 复用 `peak_hours::utc_time`），**只缺展示层**。

顺带修一个既存正确性 bug：换算按整小时做，半时区（印度 UTC+5:30 / 澳中部 +9:30）算出
非整数 `start_hour`（如 8.5）写进 JSON，Rust helper 声明 `start_hour: i32`
（`gateway/time_models.rs:59`），`serde_json::from_value` 对 8.5 解析失败 → `.ok()?`
**静默丢弃整个窗口**。半时区用户配的时段现在无声失效。

成功长相：两个时段编辑器行为一致，默认按用户本地时区展示、可切 UTC+0，半时区精确到分钟；
存储永远 UTC+0 不变；存量非整数脏数据载入时自动归一。

## 边界

### 用户 2026-07-29 拍板（契约锁定）

- [x] **tzMode 共用一个**：整个平台编辑表单单一时区开关，`peak_hours` 与 `time_models` 同步切换。
      复用 `usePlatformForm.ts:216` 现有 state（改名 `windowsTz`），非各自独立。
- [x] **换算单位改为绝对分钟**：`(hour*60 + minute ± offsetMinutes) mod 1440`，两侧同批修正。
      半时区不再产生非整数 hour。
- [x] **存量脏数据前端载入归一**：`parsePlatformPeakHours` / `parsePlatformTimeModels` 读取时把非整数
      hour 拆为 hour + minute（8.5 → hour 8, minute 30）。**无 DB migration、无 Rust 改动**，
      用户下次保存自然写回整数。

### 范围内（用户勾选三项 + 拍板的换算/脏数据）

1. 时区换算工具层：从 `formSections.tsx:453-473` 提取到 `src/utils/peakHours.ts`，改按分钟
2. `WindowsEditModal` 输入换算 + tz 切换按钮 + 「次日」提示
3. 矩阵列头 `describeWindows` 预览按 tzMode 换算 + tz 标签（`ModelsMatrixSection.tsx:37-64`）
4. 4 处硬编码中文走 i18n：`ModelsMatrixSection.tsx:35` `WEEKDAY_ZH` / `:48`「全天」/ `:60`「永不命中」
   + `WindowsEditModal.tsx:209` weekday title 数组（全库无 weekday i18n key，需新建）
5. 存量脏数据归一（`src/services/api/platforms.ts` 两处 parse）

### 范围外

- [x] 不做 tzMode 持久化（保持运行时态，默认 `local` —— 用户明确未勾）
- [x] 不改 Rust 任何代码（存储语义 UTC+0 不变，判定逻辑不变）
- [x] 不改 DB schema / 不写 migration
- [x] 不改 `manual_budget.rs` 用 `chrono::Local` 的既存不一致（与时段判定无关）
- [x] 不加 DST 跨时刻重算（沿用现有模块级 `getTimezoneOffset()` 取值时机）

### 已知约束

- [x] 换算函数**唯一真值源**，两个编辑器共用，禁抄第二份（口径漂移 = 两处显示不同基准）
- [x] `peak_hours` 侧现有行为必须零回归：`formSections.tsx:670-679` 切到新函数后，整时区用户
      看到的数字必须与改前完全一致
- [x] `formSections.tsx:595` 的 `peak_hours_desc` 文案写死「按 UTC+0 设置时段倍率」，与可切换矛盾，同批改
- [x] i18n 新 key 走顶层扁平 dotted key，注入保序（memory `locale-flat-key-convention`），
      8 语言齐平，必跑 `node scripts/check-i18n.mjs`

## 验收标准

- [x] `src/utils/peakHours.ts` 导出 `TzMode` / `localOffsetMinutes` / `utcToDisplay` / `displayToUtc`，
      按绝对分钟换算，带单测覆盖：整时区往返、半时区（+5:30）往返、跨零点 mod 1440、负偏移
- [x] `formSections.tsx` 删除本地 `LOCAL_OFFSET_HOURS`/`tzOffset`/`utcToDisplay`/`displayToUtc`，
      改 import 新模块；start/end 的 **minute 输入框也参与换算**
- [x] 整时区下 `peak_hours` 编辑器显示值与改动前逐位一致（零回归）
- [x] `WindowsEditModal` start/end 的 hour+minute 按 tzMode 双向换算，含 tz 切换按钮与「次日」提示
- [x] `describeWindows` 输出按 tzMode 换算并带 tz 标签，与弹窗内显示一致
- [x] `usePlatformForm.ts:216` state 改名 `windowsTz`，经 `PlatformEditForm.tsx` 同时透传给
      `PeakHoursSection` 与 `ModelsMatrixSection`，切换一处两处同步变
- [x] `parsePlatformPeakHours` / `parsePlatformTimeModels` 归一非整数 hour（8.5 → 8 点 30 分），
      带单测；整数值原样不动
- [x] 4 处硬编码中文全部走 i18n key，8 语言齐平，`node scripts/check-i18n.mjs` 零缺失
- [x] `peak_hours_desc` 文案不再写死「按 UTC+0」，8 语言同步
- [x] 门禁全绿：`npx tsc --noEmit` / `yarn test` / `cargo test --workspace`（应零影响，作回归证据）

## 索引

- [x] 详细设计: [design.md](design.md)
- [x] 任务/子任务/调度: task.json (`skein subtask list time-models-timezone`)

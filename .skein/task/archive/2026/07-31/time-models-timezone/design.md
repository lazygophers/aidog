# 分时段模型时区支持 — 详细设计

## 1. 工具层（唯一真值源）

放 `src/utils/peakHours.ts`（已是 `PeakWindow` 前端 helper 的归属地，不新建文件）。
从 `src/pages/platforms/formSections.tsx:453-473` 提取并把换算单位从「整小时」改为「绝对分钟」：

```ts
export type TzMode = "local" | "utc";

/** 本地时区相对 UTC 的分钟偏移（东区为正）。模块加载时取值，沿用既有时机（不做 DST 跨时刻重算）。 */
export const LOCAL_OFFSET_MINUTES = -new Date().getTimezoneOffset();

export function tzOffsetMinutes(mode: TzMode): number {
  return mode === "local" ? LOCAL_OFFSET_MINUTES : 0;
}

/** 时钟平移的纯函数内核 —— offset 显式入参，可被单测覆盖任意时区（含 +5:30 / 负偏移）。
 *  ⚠️ 单测必须打这一层：LOCAL_OFFSET_MINUTES 在模块加载时求值，
 *  vi.spyOn(Date.prototype, "getTimezoneOffset") 对它无效。 */
export function shiftClock(hour: number, minute: number, offsetMinutes: number): { hour: number; minute: number } {
  const m = (((hour * 60 + minute + offsetMinutes) % 1440) + 1440) % 1440;
  return { hour: Math.floor(m / 60), minute: m % 60 };
}

/** UTC 存值 → 选中时区显示值。按绝对分钟换算，半时区（+5:30）精确。 */
export function utcToDisplay(hour: number, minute: number, mode: TzMode) {
  return shiftClock(hour, minute, tzOffsetMinutes(mode));
}

/** 选中时区输入值 → UTC 存值。 */
export function displayToUtc(hour: number, minute: number, mode: TzMode) {
  return shiftClock(hour, minute, -tzOffsetMinutes(mode));
}
```

⚠️ **签名从「单 hour」变为「hour+minute 对」** —— 因为半时区下改 hour 会连带改 minute，
两者必须一起换算。所有 caller 的 onChange 必须同时写回 `{ start_hour, start_minute }` 两个字段。

### 脏数据归一

```ts
/** 存量非整数 hour（半时区旧逻辑产物，如 8.5）拆为 hour+minute。整数值原样返回。 */
export function normalizeWindow(w: PeakWindow): PeakWindow
```
对 `start_hour`/`end_hour` 各做一次：非整数 → `hour = Math.floor(h)`，
`minute = (w.start_minute ?? 0) + Math.round((h - hour) * 60)`，再走 `shiftClock(hour, minute, 0)` 归一进位。

## 2. 调用点改造

| 文件 | 改法 |
|---|---|
| `src/utils/peakHours.ts` | 新增上述 5 个导出 + `normalizeWindow` |
| `src/utils/peakHours.test.ts` | **新建**（该文件当前不存在）。单测全打 `shiftClock` 层：offset=0 / +330(印度) / -300(美东) / 跨零点 mod 1440 / 边界 23:59；另测 `normalizeWindow`（8.5→8:30、`8.0` 不变、已有 start_minute 叠加进位） |
| `formSections.tsx:453-473` | 删本地 4 个定义，改 import |
| `formSections.tsx:670-679` | hour 输入 onChange 同时写 minute；`:672`/`:680` 附近的 minute 输入同样接入换算 |
| `formSections.tsx:478-508` | `formatWindowPreview` 用新签名（删除「minute 不受时区影响」注释，它已被证伪） |
| `formSections.tsx:595` | `peak_hours_desc` 文案去掉写死的「按 UTC+0」，改为说明「存储 UTC+0，按下方时区显示」 |
| `services/api/platforms.ts:204-216` | `parsePlatformPeakHours` 出口 `.map(normalizeWindow)` |
| `services/api/platforms.ts:272` | `parsePlatformTimeModels` 每条 rule 的 `windows` 走 `.map(normalizeWindow)` |
| `peakHoursTz` → `windowsTz` | 引用点已 grep 全，共 5 处：`usePlatformForm.ts:120`(接口)/`:216`(state)/`:834`(返回)、`PlatformEditForm.tsx:57`(解构)/`:389`(传 PeakHoursSection) |
| `PlatformEditForm.tsx:356-365` | 给 `ModelsMatrixSection` 传 `tzMode` / `onTzModeChange` |
| `WindowsEditModal.tsx:140-160` | start/end 的 hour+minute 四个输入按 tzMode 双向换算 |
| `WindowsEditModal.tsx` 顶部 | 加 tz 切换按钮（照抄 `formSections.tsx:597-611` 的样式）+ 跨天「次日」提示 |
| `ModelsMatrixSection.tsx:37-64` | `describeWindow`/`describeWindows` 收 `tzMode` + `t`，换算后输出并带 tz 标签 |

## 3. i18n

新建 key（全库无 weekday key，`grep -n "weekday\|week_" src/locales/zh-Hans.json` 零命中）：

- `platform.weekday_short.0` … `.6`（周日→周六，7 条）—— 供 `ModelsMatrixSection.tsx:35`
  `WEEKDAY_ZH` 与 `WindowsEditModal.tsx:209` title 数组共用
- `platform.window_all_day`（「全天」，`ModelsMatrixSection.tsx:48`）
- `platform.window_never`（「永不命中」，`:60`）

8 语言齐平。既有 `platform.peak_hours_next_day` / `platform.timezone_*` 直接复用，不新建。

## 3.5 并行契约（S2/S3 同时跑，锁死边界）

`PlatformEditForm.tsx` 与 `usePlatformForm.ts` **只由 S2 改**（含给 `ModelsMatrixSection`
新增 tz props 的那一行），S3 禁碰这两个文件。二者靠以下 prop 契约对接：

```tsx
<ModelsMatrixSection ... tzMode={windowsTz} setTzMode={setWindowsTz} />
```
prop 名与类型**必须**是 `tzMode: TzMode` / `setTzMode: React.Dispatch<React.SetStateAction<TzMode>>`，
与既有 `PeakHoursSection`（`PlatformEditForm.tsx:389`）逐字一致。S3 按此签名声明接收端。

## 4. 关键取舍

| 取舍 | 选择 | 理由 |
|---|---|---|
| 换算函数放哪 | `src/utils/peakHours.ts` | 已是 `PeakWindow` 前端 helper 归属地；新建 `timezone.ts` 会把同一结构的操作拆两处 |
| 换算签名 | `(hour, minute) → {hour, minute}` | 半时区下 hour 与 minute 耦合，单 hour 签名表达不了 |
| 脏数据修哪层 | 前端 parse 层 | 用户拍板；Rust 改 f64 是为一个不再产生的格式永久兼容，代价不成比例 |
| tzMode 作用域 | 表单级共用一个 | 用户拍板；两处显示同一份窗口结构，分开切会自相矛盾 |
| tzMode 持久化 | 不做 | 用户未勾；现有 peak_hours 也是运行时态，保持一致 |
| Rust 侧 | 零改动 | 存储语义与判定逻辑均不变，纯展示层问题 |

## 5. 风险

- **peak_hours 回归**：换算函数签名变了，`formSections.tsx` 的 4 个输入框全要改。
  缓解 = 验收标准明写「整时区下显示值逐位一致」，check 阶段实测。
- **`windowsTz` 改名波及面**：`usePlatformForm.ts` 是大 hook，`peakHoursTz` 可能有多个引用点。
  缓解 = executor 先 `grep -rn "peakHoursTz" src/` 列全后再改，TS 编译会兜底捕获遗漏。
- **grill 已消解项（勿重复排查）**：`importFromPeakHours`（`ModelsMatrixSection.tsx:158`）已是
  `{ ...w }` 浅拷贝，两侧编辑不会互相污染；且两侧存储同为 UTC+0，导入语义本就正确。
- **normalizeWindow 与 `Number.isInteger` 边界**：JSON 里 `8.0` 在 JS 里 `Number.isInteger(8.0) === true`，
  不会被误拆；真浮点才走归一路径。单测须覆盖 `8.0` 不变。

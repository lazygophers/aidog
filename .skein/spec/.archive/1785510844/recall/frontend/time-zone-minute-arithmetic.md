---
title: 时区换算硬约束 — 绝对分钟精度，半时区无整数污染
layer: recall
category: frontend
keywords: [时区,换算,分钟精度,半时区,+5:30,澳门,DST,shiftClock,modulo]
source: time-models-timezone task design.md §1
authored-by: skein-spec
created: 1753805040
status: active
related: [rule-04]
updated: 1753805040
---

## 触发场景

前端时区显示/输入交互（peak_hours / time_models 的时段编辑器）需与服务端一致，半时区用户（印度 UTC+5:30、澳中部 UTC+9:30）填写时段设置。

## 陷阱：按整小时换算产生非整数 hour（旧错误）

> 半时区下，UTC 时刻 `8:00` 换到本地是 `8 + 5.5 = 13.5 小时` ，被写进 JSON 为非整数。后端 Rust 声明 `start_hour: i32`，`serde_json::from_value(13.5)` 解析失败 → `.ok()?` **静默丢弃整个窗口**，用户配置无声失效。

- ❌ 按整小时换算：`UTC 8:00 + 5:30 = 13:30` → 截断为 `hour=13, 丢失 30分钟` 或直写 `hour=13.5` → JSON 解析炸裂
- ❌ 浮点累加：浮点舍入误差累积，跨多个窗口污染

## 正解：绝对分钟 modulo 1440（硬约束，关键）

### MUST 换算公式（单位：分钟）

```ts
/** 时钟平移纯函数内核 — offset 显式入参，可被单测覆盖任意时区。 */
export function shiftClock(
  hour: number, 
  minute: number, 
  offsetMinutes: number
): { hour: number; minute: number } {
  // ✅ 绝对分钟计算：UTC 总分钟 + 偏移 → 模 1440 归一 → 拆回 hour:minute
  const m = (((hour * 60 + minute + offsetMinutes) % 1440) + 1440) % 1440;
  return { hour: Math.floor(m / 60), minute: m % 60 };
}

/** UTC 存值 → 选中时区显示值（加正偏移） */
export function utcToDisplay(hour: number, minute: number, mode: TzMode) {
  return shiftClock(hour, minute, tzOffsetMinutes(mode));
}

/** 选中时区输入值 → UTC 存值（减对应偏移） */
export function displayToUtc(hour: number, minute: number, mode: TzMode) {
  return shiftClock(hour, minute, -tzOffsetMinutes(mode));
}
```

### MUST 签名（hour+minute 必须一对处理）

- ❌ `utcToDisplay(hour, mode)` 单参数 → 无法同时拆分 minute
- ✅ `utcToDisplay(hour: number, minute: number, mode: TzMode)` 双参数

**理由**：半时区下 minute 与 hour 耦合。UTC 14:00 + 5:30 = 19:30（hour 变 19，minute 变 30）；若仅传 hour，minute 无法完整表达。所有 caller 的 onChange 必须同时写回 `{ start_hour, start_minute }` 两个字段。

### MUST 模 1440 两次（消除负偏移借位）

```ts
const m = (((hour * 60 + minute + offsetMinutes) % 1440) + 1440) % 1440;
```

- 第一次 `% 1440`：处理正偏移进位
- 加 1440：确保负数
- 第二次 `% 1440`：负偏移借位时转正

**验证**（负偏移，美东 UTC-5）：
- UTC 0:30 - 300 分钟 = -270 分钟
- `-270 % 1440 = -270`（JS 结果可能负）
- `(-270 + 1440) % 1440 = 1170` ✅ = 19:30（前一天）

## 反例 / 常见错误

| 错误                        | 为什么错                                        | 正确做法                                      |
| ----------------------------- | ----------------------------------------------- | ----------------------------------------- |
| 按小数小时换算 `13.5 → JSON` | 后端 i32 解析失败，整个窗口丢失               | 绝对分钟转整数 hour:minute                |
| 仅换算 hour，minute 不变 | 半时区下 minute 丢失，显示值错误              | hour+minute 一对换算               |
| 单次模 1440 | 负偏移时余数为负，后续计算错乱               | 双重模保证非负                            |
| 单位混淆（小时 vs 分钟）| 30 分钟错作 30 小时 → 跨多天                   | 一律用分钟，最后拆回 hour:minute |

## 落地 checklist

```bash
# 1. 验证 shiftClock 实现（必须绝对分钟）
cd src && grep -A5 "function shiftClock" utils/peakHours.ts

# 2. 验证所有 caller 用 hour+minute 对
grep -rn "utcToDisplay\|displayToUtc" src/pages/platforms/ | grep -v "minute"

# 3. 验证单测覆盖（整时区往返、半时区、负偏移、跨零点）
grep -n "describe\|it(" src/utils/peakHours.test.ts | grep -E "整时区|半时区|negative|crossing"

# 4. 验证入库清洁（无 float hour）
yarn build && grep -n "start_hour\|end_hour" src/services/api/platforms.ts | grep parse
```

## 验证场景

- 北京用户（UTC+8，整时区）：UTC 14:00 → 显示 22:00（0 舍入误差）
- 印度用户（UTC+5:30，半时区）：UTC 8:00 → 显示 13:30（**精确到分钟**，无 8.5 污染）
- 美东用户（UTC-5，负偏移）：UTC 0:30 → 显示 19:30（前一天，正确借位）
- 跨零点：UTC 23:50 + 60 分钟 → 00:50（第二天，正确进位）

## 适用

- 时段编辑器（peak_hours / time_models）时区展示/输入
- 任何需要精确分钟级时区换算的前端交互

## 关联

[[rule-04]] (i18n key 齐平) · [[dirty-float-hour-normalization]] (脏数据拆分)

## 案例

- time-models-timezone task (commit 7f78c93e) — peak_hours 侧从整小时改绝对分钟换算 + half-hour 支持
- WindowsEditModal 时区双向换算 (commit 8fb99885) — time_models 侧接入相同规则

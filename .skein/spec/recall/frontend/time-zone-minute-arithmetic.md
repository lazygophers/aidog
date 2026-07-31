---
title: time-zone-minute-arithmetic
name: time-zone-minute-arithmetic
description: 时区换算硬约束 — 绝对分钟精度
layer: recall
keywords: [时区,换算,分钟,精度]
created: 1785516136
inclusion: auto
---

## time-zone-minute-arithmetic

## 时区换算硬约束 — 绝对分钟精度

前端时区显示/输入交互需与服务端一致，半时区用户（印度 UTC+5:30 等）填写时段时必须绝对分钟精度。

## MUST 换算公式（单位：分钟）

```ts
export function shiftClock(
  hour: number, 
  minute: number, 
  offsetMinutes: number
): { hour: number; minute: number } {
  // ✅ 绝对分钟计算：UTC 总分钟 + 偏移 → 模 1440 归一 → 拆回 hour:minute
  const m = (((hour * 60 + minute + offsetMinutes) % 1440) + 1440) % 1440;
  return { hour: Math.floor(m / 60), minute: m % 60 };
}

export function utcToDisplay(hour: number, minute: number, mode: TzMode) {
  return shiftClock(hour, minute, tzOffsetMinutes(mode));
}

export function displayToUtc(hour: number, minute: number, mode: TzMode) {
  return shiftClock(hour, minute, -tzOffsetMinutes(mode));
}
```

## 陷阱：按整小时换算产生非整数

半时区下 UTC `8:00` 换到本地是 `8 + 5.5 = 13.5 小时`，被写进 JSON 后后端解析失败 → 静默丢弃。

- ❌ 按整小时换算：UTC 8:00 + 5:30 = 13:30 → 截断为 hour=13, 丢失分钟
- ❌ 直写 hour=13.5 → JSON 解析炸裂

## 适用

- 前端时区显示/输入交互（peak_hours / time_models 编辑器）
- 任何跨时区时刻换算

## 关联

[[dirty-float-hour-normalization]]、[[form-level-tz-state-sharing]]

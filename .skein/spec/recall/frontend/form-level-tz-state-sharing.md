---
title: form-level-tz-state-sharing
name: form-level-tz-state-sharing
description: 表单级时区状态共用 — 单一 state 透传避免口径漂移
layer: recall
keywords: [表单,时区,状态共用,peak_hours]
created: 1785516136
inclusion: auto
---

## form-level-tz-state-sharing

## 表单级时区状态共用 — 单一 state 透传避免口径漂移

同一表单内多个组件展示同一类数据不同维度时，需要单一 state 透传避免口径漂移。

## 陷阱：各组件独立 state 导致口径漂移

PlatformEditForm 编辑单个平台。peak_hours 与 time_models 都含「时段」结构，都需时区显示切换。若各自独立 state：
- 用户在 peak_hours 切到 UTC+0
- time_models 仍显示本地时区
- 同一时段在两处显示基准不同 → 用户困惑

❌ 各自独立 state（禁用）
```tsx
<PeakHoursSection tzMode={peakHoursTz} setTzMode={setPeakHoursTz} />
<ModelsMatrixSection tzMode={modelsTz} setTzMode={setModelsTz} />
```

## MUST 单一真值源（表单级 state）

✅ **表单级单一 state 透传**

```ts
// usePlatformForm.ts：表单级 hook
export function usePlatformForm(...): PlatformFormState {
  // ✅ 时区展示模式：表单级单一 state（默认本地）
  const [windowsTz, setWindowsTz] = useState<"local" | "utc">("local");
  
  return {
    // ...
    peakHours, setPeakHours, windowsTz, setWindowsTz,  // 单一 state 对外透传
  };
}
```

## 适用

- 表单内多个子组件需同步状态的场景
- peak_hours + time_models 编辑器一致性

## 关联

[[time-zone-minute-arithmetic]]、[[dirty-float-hour-normalization]]

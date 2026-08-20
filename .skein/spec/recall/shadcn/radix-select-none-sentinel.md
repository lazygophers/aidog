---
title: radix-select-none-sentinel
name: radix-select-none-sentinel
description: radix Select 空值哨兵模式
layer: recall
keywords: [radix,Select,空值,哨兵]
created: 1785516136
inclusion: auto
---

## radix-select-none-sentinel

## radix Select 空值哨兵模式

使用 radix Select 组件时，value 属性需要处理空值/undefined 状态，使用哨兵值避免内部验证错误。

## 陷阱-正解

❌ **陷阱**：直接使用 `value=""` 会触发 radix Select 内部验证错误（SelectItem value="" 会抛错）。
✅ **正解**：使用 `__none__` 哨兵值 + onValueChange 映射回 undefined/""。

## 模式模板

```tsx
// 定义哨兵常量
const NONE = "__none__";

// 组件使用
<Select
  value={!value ? NONE : value}
  onValueChange={(v) => onChange(v === NONE ? undefined : v)}
>
  <SelectContent>
    <SelectItem value={NONE}>—</SelectItem>
    {opts.map((o) => <SelectItem key={o} value={o}>{o}</SelectItem>)}
  </SelectContent>
</Select>
```

## 适用

- radix Select 组件（@/components/ui/select）
- 需要空值占位符的下拉选择场景

## 案例

- `src/pages/platforms/PlatformPicker.tsx:105-109` 可选平台选择器

## 关联

[[radix-select-number-mapping]]

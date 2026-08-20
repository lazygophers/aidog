---
title: radix-select-number-mapping
name: radix-select-number-mapping
description: radix Select number 双向映射
layer: recall
keywords: [radix,Select,number,String]
created: 1785516136
inclusion: auto
---

## radix-select-number-mapping

## radix Select number 双向映射

radix Select 的 value 属性只接受 string 类型，需要处理 number 类型数据时使用双向映射。

## 陷阱-正解

❌ **陷阱**：直接传 number 会触发类型错误或运行时异常。
✅ **正解**：存储/显示时 String() 转字符串，回调时 Number() 转回数字。

## 模式模板

```tsx
<Select
  value={String(numberValue)}  // 存储/显示：number → string
  onValueChange={(v) => onChange(Number(v))}  // 回调：string → number
>
  <SelectContent>
    {options.map((n) => <SelectItem key={n} value={String(n)}>{n}</SelectItem>)}
  </SelectContent>
</Select>
```

## 适用

- radix Select value 仅收 string（类型约束）
- 需要处理 number 选项的分页器/数值选择器

## 案例

- `src/pages/Logs/primitives.tsx:374` Pagination pageSize: `value={String(pageSize)}` + `onValueChange={v => onPageSizeChange(Number(v))}`

## 关联

[[radix-select-none-sentinel]]

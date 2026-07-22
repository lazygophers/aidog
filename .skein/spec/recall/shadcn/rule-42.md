---
title: radix Select number 双向映射
layer: recall
category: shadcn
keywords: [radix,Select,number,String,Number,双向映射]
source: -
authored-by: skein-spec
created: 1784730563
status: active
related: []
updated: 1784730563
---

## 触发场景
radix Select 的 value 属性只接受 string 类型，需要处理 number 类型数据。

## 陷阱-正解
❌ **陷阱**：直接传 number 会触发类型错误或运行时异常。
✅ **正解**双向映射：存储/显示时 String() 转字符串，回调时 Number() 转回数字。

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

## 关联
[[shadcn-select-none-sentinel]]

## 案例
- `src/pages/Logs/primitives.tsx:374` Pagination pageSize: `value={String(pageSize)}` + `onValueChange={v => onPageSizeChange(Number(v))}`

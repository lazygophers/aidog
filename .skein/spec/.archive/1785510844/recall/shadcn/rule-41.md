---
title: radix Select 空值哨兵模式
layer: recall
category: shadcn
keywords: [radix,Select,空值,哨兵,__none__]
source: -
authored-by: skein-spec
created: 1784730556
status: active
related: []
updated: 1784730556
---

## 触发场景
使用 radix Select 组件时，value 属性需要处理空值/undefined 状态。

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

## 关联
[[rule-42]]

## 案例
- `src/pages/Logs/primitives.tsx:12-13` 定义 NONE 常量 + 注释说明
- `src/components/settings/editors/EnvEditor.tsx:55-58` 应用哨兵模式

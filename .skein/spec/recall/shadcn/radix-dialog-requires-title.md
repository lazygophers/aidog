---
title: radix-dialog-requires-title
name: radix-dialog-requires-title
description: Radix Dialog 必须含 DialogTitle
layer: recall
keywords: [Radix,Dialog,DialogTitle,a11y]
created: 1785516136
inclusion: auto
---

## radix-dialog-requires-title

## Radix Dialog 必须含 DialogTitle

Radix Dialog 组件必须包含 DialogTitle 以满足无障碍（a11y）要求。自定义 header 时使用 sr-only 隐藏 title。

## MUST 硬约束

Radix Dialog **必须包含 DialogTitle**，否则会触发 a11y 警告。

## 实现模式

❌ **陷阱**：自定义 header 时完全省略 DialogTitle，破坏 a11y。
✅ **正解**：用 `sr-only` className 隐藏 DialogTitle，保留语义但不破坏自定义 header 视觉。

## 模式模板

```tsx
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";

<Dialog open={open} onOpenChange={onOpenChange}>
  <DialogContent>
    {/* sr-only title 满足 Radix Dialog a11y 要求 */}
    <DialogTitle className="sr-only">{title}</DialogTitle>
    
    {/* 自定义 header */}
    <div style={{ display: "flex", justifyContent: "space-between" }}>
      <div>{title}</div>
      <Button onClick={onClose}>×</Button>
    </div>
  </DialogContent>
</Dialog>
```

## 适用

- 所有 Radix Dialog 用法（@/components/ui/dialog）
- 需要完全自定义 header 视觉的场景

## 案例

- `src/components/settings/editors/StatusLineSection/SegmentEditModal.tsx:49-50` sr-only title + 自定义 header

## 关联

[[dialog-open-explicit-null]]

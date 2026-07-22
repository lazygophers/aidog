---
title: Radix Dialog 必须含 DialogTitle
layer: core
category: shadcn
keywords: [Radix,Dialog,DialogTitle,a11y,sr-only,无障碍]
source: -
authored-by: skein-spec
created: 1784730580
status: active
related: []
updated: 1784730580
---

## 触发场景
使用 Radix Dialog 组件时，必须满足无障碍（a11y）要求。

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
    {/* sr-only title 满足 Radix Dialog a11y 要求，不破坏自定义 header 视觉 */}
    <DialogTitle className="sr-only">{title}</DialogTitle>
    
    {/* 自定义 header */}
    <div style={{ display: "flex", justifyContent: "space-between" }}>
      <div>{title}</div>
      <Button onClick={onClose}>×</Button>
    </div>
    
    {/* ... 其他内容 ... */}
  </DialogContent>
</Dialog>
```

## 适用
- 所有 Radix Dialog 用法（@/components/ui/dialog）
- 需要完全自定义 header 视觉的场景

## 关联
[[shadcn-dialog-async-open]]

## 案例
- `src/components/settings/editors/StatusLineSection/SegmentEditModal.tsx:49-50` sr-only title + 自定义 header

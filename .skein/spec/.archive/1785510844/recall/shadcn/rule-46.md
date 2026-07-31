---
title: shadcn Button cva 基类压 svg 16px
layer: recall
category: shadcn
keywords: [shadcn,Button,cva,svg,16px,size-4]
source: -
authored-by: skein-spec
created: 1784730624
status: active
related: []
updated: 1784730624
---

## 触发场景
shadcn Button 组件 cva 基类含 `[&_svg]:size-4` 规则，统一压内部 svg 至 16px。

## MUST 硬约束
shadcn Button 内的 svg 图标会被强制压至 16px（`size-4` = 1rem = 16px），自定义尺寸需显式覆盖。

## 实现模式
```tsx
// Button cva 基类（shadcn/ui/button.tsx）
variants: {
  // ...
  base: "inline-flex items-center justify-center rounded-md text-sm font-medium ring-offset-background transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 [&_svg]:size-4"
}
```

## 适用
- 所有 shadcn Button 用法（@/components/ui/button）
- nav icon 等小图标场景（接受 16px 默认）

## 关联
[[rule-43]]

## 案例
- shadcn-pages task：Sidebar nav icon 迁移至 Button，接受 16px 默认

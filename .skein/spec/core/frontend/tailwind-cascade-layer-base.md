---
name: tailwind-cascade-layer-base
title: UA reset/全局元素规则必须 @layer base
layer: core
category: frontend
keywords: [tailwind,layer,css,reset,global]
created: 1725080438
inclusion: auto
---

## 硬约则

`src/styles/globals.css` 中 UA reset 和全局元素规则（如 body/html）MUST 用 `@layer base` 包裹。

Tailwind v4 禁用 v3 三行 `@tailwind` 指令，改用 `@layer` 分层：

```css
@layer base {
  body { @apply bg-background text-foreground; }
}
```

## 验收

- `src/styles/globals.css:4-6` 无任何 `@tailwind` 指令
- `package.json:89 tailwindcss ^4.3.3` v4+

## 禁用

❌ v3 风格 `@tailwind base/components/utilities` → v4 语法错误

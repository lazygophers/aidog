---
title: css-reset-layer
layer: recall
category: style
keywords: [tailwind,v4,reset,layer,css-precedence,shadcn]
status: active
---

## CSS reset 必须写进 @layer base

## 触发场景

Tailwind v4 迁移到新组件库（如 shadcn）后，CSS reset 声明失效，导致按钮/输入框文字贴边。症状表现为 `px-4 py-2` / `px-3 py-1` 等 utility 类被反压。

## 陷阱 & 正解

❌ **陷阱**：在 `src/styles/globals.css` 裸写 CSS reset

```css
* { padding: 0; margin: 0; }
button, input, select, textarea { color: inherit; }
```

Tailwind v4 把 utilities 放在 `@layer utilities`（layered 声明），而**unlayered 声明优先级高于所有 layered 声明**（与特异性无关）。结果裸 `*` 反压所有 utility 类，shadcn 组件的 padding 全失效。

✅ **正解**：reset 必须写进 `@layer base`，与 Tailwind 系统同层级

```css
@layer base {
  * {
    @apply p-0 m-0;
  }
  button, input, select, textarea {
    @apply text-inherit;
  }
}
```

或使用 `@layer` 包裹任何 reset 声明，确保与 v4 utilities 优先级对等。

## 反例（错误模式）

| ❌ 错 | ✅ 改为 |
|---|---|
| `* { padding: 0; }` unlayered | `@layer base { * { @apply p-0; } }` |
| `@tailwind base; @tailwind components; @tailwind utilities;` v3 方式 | `@import "tailwindcss";` 单行 v4 方式 |
| reset 写在 @import 之后（加载顺序混乱） | reset @layer base 写在 Tailwind import 之前 |

## 案例

commit `2b14131e`：git diff 展示 `src/styles/globals.css` 把旧 `* { padding: 0; }` 块和 `button/input/... { color: inherit }` 块改为 `@layer base { ... }` 包裹。migrated shadcn 组件（`CompactCard/StatChip/BalanceBar` 等）padding 随即生效，测试通过。

## 适用

- Tailwind v3 → v4 迁移
- 新项目用 v4 + shadcn
- 任何 CSS reset 失效症状排查

## 关联

[[shadcn-infra-30]] [[shadcn-infra-02]] [[tailwind-cascade-layer-unlayered]]

---
title: tailwind-cascade-layer-unlayered
layer: recall
category: frontend
keywords: [tailwind,cascade-layer,unlayered,layer,preflight,cascade,css]
status: active
---

## CSS cascade layer: 裸写规则反压 layer 内 utility

## 判据
Tailwind v4 项目里若 `globals.css` 顶部声明了 `@layer theme, base, components, utilities;` 并用 `@import "tailwindcss/utilities" layer(utilities)`（分层导入而非单行 `@import "tailwindcss";`），则**任何裸写（不在 `@layer` 块内）的 CSS 规则优先级都高于任何 layered 声明，与选择器特异性无关**（CSS cascade layer 规范：unlayered > 任意 layer）。

## 陷阱
补 preflight 缺失的 UA reset（如 `button,input,select,textarea { color: inherit }`）时若裸写在 globals.css 顶层（不包 `@layer base {}`），会反压 utilities 层——`<button>` 元素上所有 `text-*-foreground` 等 utility class 全部失效（被裸写规则盖过，尽管 utility class 特异性更高）。

## 正解
补 UA reset 规则必须包进 `@layer base { }`，与 globals.css 顶部声明的 layer 顺序对齐，禁裸写元素选择器规则。

## 检查
globals.css 顶部若见 `@layer <names>;` 声明 + `@import ... layer(...)`，改动前先确认新增规则是否包在对应 `@layer` 块内。

## 案例
frontend-compositing-purge task：commit c3f9515e 裸写 UA reset 引入 button 文字色失效缺陷 → ce3d5dd5 改为 `@layer base {}` 包裹修复。

## 适用
Tailwind v4 + cascade layer 项目，补 preflight/UA reset 规则、新增全局元素选择器样式时。

---
title: theme-dark-class-dead-code
layer: recall
category: frontend
keywords: [theme,dark,applyTheme,data-mode,classList,tailwind,dark-mode,color-scheme]
status: active
---

## 本项目主题机制: data-mode 属性驱动, dark: utility 死代码

## 判据
本项目主题机制：`src/themes/index.ts::applyTheme` 只做两件事——`applyThemeVars` 写 **inline style** + `setAttribute("data-mode", mode)`。**从不 `classList.add("dark")`**。

## 陷阱
故 globals.css 里 `@custom-variant dark (&:is(.dark *))` 与 `.dark {}` token 块全是死代码，任何 `dark:` Tailwind utility class 在本项目**永不生效**（`.dark` 选择器条件永不成立）。已知残留 2 处：`field.tsx:120`、`alert.tsx:13`。

## 正解
判本项目深色态必须看 `mono.ts` 的 `dark` 块或 `:root[data-mode="dark"]` 选择器，禁用 `dark:` Tailwind utility class。同族约束：`color-scheme` CSS 属性不能写 `light dark`（那是跟随系统主题），必须随 `data-mode` 显式覆盖对应值。

## 案例
frontend-compositing-purge task 审计发现 `field.tsx:120`、`alert.tsx:13` 残留失效 `dark:` class。

## 适用
本项目（aidog）任何涉及深色态样式判断/新增组件暗色适配时；planning 阶段先查 `src/themes/index.ts::applyTheme` 确认主题切换机制再动笔，勿凭 Tailwind 惯例假设 `.dark` classList 生效。

## 关联
[[shadcn-infra-31]]（同类 shadcn 主题运行时切换技巧，本条补充"本项目未用 classList 切换，dark: utility 死代码"这一具体陷阱）

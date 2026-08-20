---
title: planning-scope-pregrep
name: planning-scope-pregrep
description: planning 范围预筛纪律（grep）
layer: recall
keywords: [planning,预筛,grep,范围]
created: 1785516136
inclusion: auto
---

## planning-scope-pregrep

## planning 范围预筛纪律（grep）

planning 阶段需要预先用 grep 检查目标范围是否真的需要该改动，避免对不含相关代码的文件跑不必要的改造。

## 陷阱-正解

❌ **陷阱**：planning 时未预筛，按通用模板对所有目标域跑变更逻辑，对不含相关代码的区域产生误判。
✅ **正解**：planning 先 grep 预筛，检查目标域是否真的含需要改造的代码；命中 0 即跳过。

## 预筛命令模板

```bash
# 检查是否存在相关代码
grep -r "相关代码模式" 目标路径/ --include="*.ts*"

# 命中 0 → 跳过该域的改造
```

## 例子

- **shadcn 迁移**：检查是否有 `<button` / `<input` / `<select` 等表单控件
  ```bash
  grep -c "<button\|<input\|<select\|<textarea" src/pages/PopoverConfigTab/*.tsx
  # 命中 0 → 跳过
  ```
- **时区处理**：检查是否有 `new Date()` 或时间计算
  ```bash
  grep -r "new Date()\|getTime\|setHours" src/ --include="*.tsx"
  ```

## 适用

- planning 阶段大范围变更（框架升级、组件库迁移、业务重构）
- 确保 task 范围精准，避免 false positive

## 关联

[[radix-select-none-sentinel]]、[[platform-creation-entry-consolidation]]

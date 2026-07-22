---
title: popover 独立窗口只读域跳过 shadcn 迁移
layer: recall
category: shadcn
keywords: [popover,只读,shadcn,迁移,预筛,grep]
source: -
authored-by: skein-spec
created: 1784730614
status: active
related: []
updated: 1784730614
---

## 触发场景
popover 独立窗口（TrayConfigTab）是只读展示域，无表单控件，不适用通用 shadcn 迁移模板。

## 陷阱-正解
❌ **陷阱**：planning 阶段未预筛，按通用模板对所有页面跑 shadcn 迁移，对只读域产生误判（实际无 button/form 可迁）。
✅ **正解**：planning 先 grep 预筛，检查目标域是否含表单控件（`<button`/`<input`/`<select` 等）；命中 0 即跳过。

## 预筛命令
```bash
# 检查目标域是否有可迁组件
grep -c "<button\|<input\|<select\|<textarea" src/pages/PopoverConfigTab/*.tsx

# 命中 0 → 跳过 shadcn 迁移
```

## 适用
- popover 独立窗口（TrayConfigTab）等只读域
- planning 阶段 shadcn 迁移范围判定

## 关联
[[shadcn-select-none-sentinel]]

## 案例
- shadcn-pages task：PopoverConfigTab 经 grep 命中 0，确认无需迁移

---
title: PlatformEditForm Modal 架构模式
layer: recall
category: frontend
keywords: [modal, state, architecture, PlatformEditForm, usePlatformForm, PlatformPasteCtx, CpaImportModal, SmartPasteModal]
source: cpa-import-group-missing
authored-by: skein-memory
created: 1783832115
---

# PlatformEditForm Modal 架构模式

何时被读: 在 PlatformEditForm 加新 modal 时（如 Sub2Api 等）
谁读: 前端开发者 / sub-agent
不遵守的代价: 架构不一致 → modal state 位置混乱 → 后续维护成本↑

## 两类 Modal 区分

### 直接灌表单 Modal（SmartPasteModal 模式）
- **State 位置**: `usePlatformForm` hook 内定义 `showPaste` + `setShowPaste`
- **传递方式**: 通过 `PlatformPasteCtx` 传递 `setShowPaste` 给 `applyPaste` 等函数
- **关闭时机**: `onClose` 直接调用 `setShowPaste(false)`（modal 组件内处理）
- **适用场景**: Modal 的 onApply 直接操作表单字段（灌入 name/apiKey/models 等）

### 跨表单 Modal（CpaImportModal 模式）
- **State 位置**: `PlatformEditForm` 组件本地定义 `showCpaImport`（**不在 hook 内**）
- **传递方式**: **不加进 PlatformPasteCtx**
- **关闭时机**: `onApplied` 回调内由调用方处理 `setShowCpaImport(false)`（modal 返回原始数据，父级决策）
- **适用场景**: Modal 的 onApplied 返回中间数据（如 `MappedPlatform[]`），由父级分派逻辑（单条灌表单 vs 多条批量创建）

## 架构原则

1. **Modal 直接操作表单字段 → state 放 hook，通过 PlatformPasteCtx 传 setter**
2. **Modal 返回中间数据由父级决策 → state 放组件本地，关闭由 onApplied 回调处理，不加 PlatformPasteCtx**

## 后续新 Modal 决策树

```
新 Modal (如 Sub2Api)
├─ onApply 直接填表单字段？
│  └─ 是 → SmartPasteModal 模式（state 在 hook + PlatformPasteCtx）
└─ 否（返回中间数据由父级决策）？
   └─ 是 → CpaImportModal 模式（state 在组件本地 + onApplied 关闭）
```

## 验收

- [ ] grep `showCpaImport` / `showPaste` 在 PlatformEditForm 组件本地定义
- [ ] grep `PlatformPasteCtx` 不含跨表单 modal 的 state setter
- [ ] 跨表单 modal 的 `onApplied` 回调包含 `setShow<Modal>(false)` 关闭逻辑

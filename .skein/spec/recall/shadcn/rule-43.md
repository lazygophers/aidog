---
title: Dialog.open 需显式 null 判断
layer: recall
category: shadcn
keywords: [Dialog,open,null,Promise,resolve,bool]
source: -
authored-by: skein-spec
created: 1784730571
status: active
related: []
updated: 1784730571
---

## 触发场景
Dialog.open 属性需要 bool 类型，但实际控制常来自 Promise resolve 型 state（如 `{resolve}|null`）。

## 陷阱-正解
❌ **陷阱**：直接用 `open={modalState}` 会将 null/对象转为 bool，无法正确反映「有 state 即打开」语义。
✅ **正解**：`open={modalState !== null}` 显式判断，确保 null 关闭、非空打开。

## 模式模板
```tsx
const [modalState, setModalState] = useState<{resolve: (v: any) => void} | null>(null);

<Dialog open={modalState !== null} onOpenChange={(o) => { if (!o) setModalState(null); }}>
  <DialogContent>
    {/* ... modal 内容 ... */}
  </DialogContent>
</Dialog>
```

## 适用
- 任何 Promise resolve 型 state 控制弹窗开关的场景（如 async confirm/自定义 Modal）
- Radix Dialog open 属性需要 bool 的场景

## 关联
[[shadcn-select-none-sentinel]]

## 案例
- 通用模式：shadcn-pages 迁移中所有 Dialog 均用 `open={state !== null}`

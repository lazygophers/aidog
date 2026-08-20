---
title: dialog-open-explicit-null
name: dialog-open-explicit-null
description: Dialog.open 需显式 null 判断
layer: recall
keywords: [Dialog,open,null,Promise]
created: 1785516136
inclusion: auto
---

## dialog-open-explicit-null

## Dialog.open 需显式 null 判断

Dialog.open 属性需要 bool 类型，当使用 Promise resolve 型 state 时需显式 null 判断。

## 陷阱-正解

❌ **陷阱**：直接用 `open={modalState}` 会将 null/对象转为 bool，无法正确反映语义。
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

[[radix-dialog-requires-title]]

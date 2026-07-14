---
title: auth-dir 拖拽目标识别（WKWebView 不可靠 best-effort）
layer: recall
category: frontend
keywords: [authdir,dragtarget,ondragenter,wkwebview,best-effort,退化,DOM target]
source: cpa-drag-import
authored-by: skein-memory
created: 1784035659
---

# auth-dir 拖拽目标识别（WKWebView 不可靠 best-effort）

何时被读: 需区分拖入落到 modal 哪个子区域（如源根目录 vs auth 凭据目录）时
不遵守代价: 拖入误识别目标 → 导入到错误区域

## 问题: Tauri onDragDropEvent 无 DOM target

`onDragDropEvent` 是 webview 级事件，payload **不含 DOM target 信息**，无法区分拖到 modal 哪个子区域。

## 模式: HTML5 onDragEnter 标记 + Tauri drop 读 ref

```typescript
const dragTargetRef = useRef<"source" | "authdir">("source");

// auth-dir 按钮绑 HTML5（default source）
<Button
  onDragEnter={() => { dragTargetRef.current = "authdir"; }}
  onDragLeave={() => { dragTargetRef.current = "source"; }}
>选择认证目录</Button>

// Tauri drop 读 ref 决定去向
if (type === "drop") {
  const target = dragTargetRef.current;
  if (target === "authdir") setAuthDir(paths[0]);
  else handleDropSources(paths);
}
```

## WKWebView 退化（best-effort）

macOS WKWebView HTML5 `drop` 不触发，`onDragEnter` **可能同病**（未实测）。若 onDragEnter 也不触发 → dragTargetRef 恒 "source" → auth-dir 拖入**退化回 dialog 选目录**，源拖拽（主路径）完全不受影响。

- 此方案 best-effort：能提升则提升，失效不影响核心功能
- **禁依赖 dragTargetRef 做硬约束**：失效路径必须可回退 dialog
- onDragEnter/onDragLeave 必须配对

## 关联

- core/frontend/tauri-drag-drop-api.md（依赖）

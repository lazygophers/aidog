---
title: Tauri 拖拽事件 API（macOS WKWebView 限制）
layer: core
category: frontend
keywords: [tauri,drag,drop,wkwebview,html5,ondragdropevent,跨平台,onDrop]
source: cpa-drag-import
authored-by: skein-memory
created: 1784035658
---

# Tauri 拖拽事件 API（macOS WKWebView 限制）

何时被读: Tauri 前端实现文件拖拽导入时
不遵守代价: macOS WKWebView 拖拽完全失效 → 用户无法拖入文件

## MUST 用 Tauri onDragDropEvent，禁 HTML5 onDrop

macOS WKWebView 的 HTML5 `drop` 事件**不触发**（已知限制）。Tauri `getCurrentWebview().onDragDropEvent()` 绕过此限制，提供原生级拖拽事件。

## 范本（ImportExportTab.tsx:271-306 + CpaImportModal.tsx:238）

```typescript
useEffect(() => {
  let unlisten: (() => void) | undefined;
  let cancelled = false;
  getCurrentWebview()
    .onDragDropEvent((event) => {
      const { type } = event.payload;
      const paths = (event.payload as { paths?: string[] }).paths ?? [];
      if (type === "enter" || type === "over") {
        if (type === "enter") setDragActive(paths.some(p => 目标判断));
      } else if (type === "drop") {
        setDragActive(false);
        // paths[] 处理
      } else { // leave / cancel
        setDragActive(false);
      }
    })
    .then((fn) => { if (cancelled) fn(); else unlisten = fn; })
    .catch(() => {});
  return () => { cancelled = true; unlisten?.(); };
}, [isOpen]);
```

## event.payload.type

- `enter`/`over`: paths[] → 高亮判断
- `drop`: paths[] → 取目标文件
- `leave`/`cancel`: 清高亮

## 约束

- **禁混 HTML5 onDrop**: macOS WKWebView 不触发 drop
- **MUST unlisten**: cleanup 调 unlisten()，否则泄漏
- **listener 依赖最小化**: 闭包经 ref 同步(handleDropRef.current = handleDropSources)，listener 生命周期只跟 isOpen，避免 state churn 致频繁 re-listen

## 验收

- [ ] grep `onDrop|onDragOver` 在拖拽组件 0 命中（HTML5 API）
- [ ] grep `onDragDropEvent` ≥1 命中
- [ ] cleanup unlisten 存在

## 关联

- recall/frontend/auth-dir-target-drag.md（target 识别依赖此 core）

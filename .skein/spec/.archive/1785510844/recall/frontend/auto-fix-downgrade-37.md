---
title: Tauri 拖拽事件 API（macOS WKWebView 限制）
layer: recall
category: frontend
keywords: [tauri,drag,drop,wkwebview,html5,ondragdropevent]
source: auto-fix-downgrade
authored-by: skein-spec
created: 1784706931
status: active
related: []
updated: 1784706931
---

# Tauri 拖拽事件 API（macOS WKWebView 限制）

## 触发场景
Tauri 前端实现文件拖拽导入时。

## MUST 用 Tauri onDragDropEvent，禁 HTML5 onDrop
macOS WKWebView 的 HTML5 `drop` 事件不触发。Tauri `getCurrentWebview().onDragDropEvent()` 绕过此限制。

## 范本
```typescript
useEffect(() => {
  let unlisten: (() => void) | undefined;
  let cancelled = false;
  getCurrentWebview()
    .onDragDropEvent((event) => {
      const { type } = event.payload;
      const paths = (event.payload as { paths?: string[] }).paths ?? [];
      if (type === "drop") {
        // paths[] 处理
      }
    })
    .then((fn) => { if (cancelled) fn(); else unlisten = fn; })
  return () => { cancelled = true; unlisten?.(); };
}, [isOpen]);
```

## event.payload.type
- enter/over: paths[] → 高亮判断
- drop: paths[] → 取目标文件
- leave/cancel: 清高亮

## 约束
- 禁混 HTML5 onDrop（macOS WKWebView 不触发）
- MUST unlisten（cleanup 调 unlisten()，否则泄漏）
- listener 依赖最小化（避免 state churn 致频繁 re-listen）

## 适用
Tauri 文件拖拽导入、跨平台拖拽

## 关联
[[modal-state-architecture]] (Tauri UI 约束)

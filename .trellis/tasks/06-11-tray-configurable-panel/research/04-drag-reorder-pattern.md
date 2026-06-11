# Research: 拖拽排序模式（复用 group 模式）

- **Query**: 复用刚做的 group 拖拽模式做 tray item 排序
- **Scope**: 内部前端
- **Date**: 2026-06-11

## 现有模式：原生 HTML5 Drag and Drop（无第三方库）

`src/pages/Groups.tsx` 两处实现，可直接照搬：

### A. group 列表整体重排（最贴合 tray items 用例）
`Groups.tsx:581-592`：
```
draggable
onDragStart={e => { setGroupDragIdx(i); e.dataTransfer.effectAllowed = "move"; }}
onDragOver={e => { e.preventDefault(); e.dataTransfer.dropEffect = "move"; if (...) setDragOverIndex(i); }}
onDrop={e => {
  e.preventDefault();
  const reordered = [...details];
  const [moved] = reordered.splice(groupDragIdx, 1);
  reordered.splice(i, 0, moved);
  setDetails(reordered);
  groupApi.reorder(reordered.map(d => d.group.id)).catch(console.error);  // 整体顺序落库
}}
```

### B. selected platforms 重排（组内优先级）
`Groups.tsx:354-366` + helper `reorderPlatforms(from,to)`（:96）。状态：`dragIndex`（:88）、`dragOverIndex`。视觉态 `isDragging`/`isDragOver`（:358-359）。

## 对应到 tray items

- state：`dragIndex` / `dragOverIndex`。
- onDrop：splice 重排 items 数组 → setState → `settingsApi.set("tray","config", {...items})` → 触发后端 refresh 托盘。
- **排序即数组顺序**（与 group_reorder 一致），data model 不需独立 order 字段（见 02）。

## 后端 reorder 先例（如需后端排序持久化）

`group_reorder(ordered_ids)` → `db::reorder_groups`（lib.rs:282，db sort_order 列 Migration 006）。
tray 不需此机制——tray config 整体存 settings JSON，前端拖完整存即可，无需后端 reorder 命令。

## Caveats

- 原生 DnD 在触摸屏 / 某些 webview 体验一般，但项目已用此模式（一致性优先）。

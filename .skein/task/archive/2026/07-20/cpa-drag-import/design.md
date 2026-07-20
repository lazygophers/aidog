# CPA 配置导入拖拽支持 — 详细设计

## 数据流(拖拽路径)
```
用户拖文件入 modal
  → Tauri onDragDropEvent payload.type="enter"/"over" → setDragActive(true) + modal 高亮
  → payload.type="drop" → paths[] = event.payload.paths
       → 读 dragTargetRef.current:
            "authdir" → setAuthDir(paths[0]) (HTML5 onDragEnter 识别; 不可靠则此分支不触发, 走 source)
            "source"(默认) → handleDropSources(paths[])
  → payload.type="leave"/"cancel" → setDragActive(false)

handleDropSources(paths[]):
  for path in paths:
    r = await cpaImportApi.parse(path, authDir || undefined)
    累加: originals/order/rows 增量合并(去重 rowId = idx::name::base_url, idx 取当前 order.length 偏移)
    sourceFiles.push(...r.source_files); skipped.push(...r.skipped)
  setParsing 协调(任一在途则 true)
```

## 关键取舍

### A. 拖拽事件: Tauri onDragDropEvent(非 HTML5 DnD)
- 原因: macOS WKWebView HTML5 onDrop 不触发(ImportExportTab:271 实证)
- 范本: ImportExportTab:271-306(getCurrentWebview().onDragDropEvent, unlisten 卸载)
- CpaImportModal 内挂一个 useEffect 注册 onDragDropEvent, isOpen 时生效, 关闭时 unlisten(或常驻判 isOpen)

### B. 多源叠加: 前端循环调 parse + 累加
- 后端 cpa_import_parse 单 path 不改签名
- 前端 handleDropSources 逐个 await parse(串行, 避并发压后端; 单批通常 ≤10 文件可接受)
- 累加去重: rowId = `${baseIdx}::${name}::${base_url}`, baseIdx 用当前 order.length 作偏移避免跨源撞 idx
- sourceFiles/skipped 数组 concat(不去重, 保留全量审计)
- 与现有 handleParse(单源 dialog)共存: 抽取 parseAndMerge(path) 共用累加逻辑, handleParse 调单次, handleDropSources 循环

### C. auth-dir target 识别(HTML5 onDragEnter + Tauri drop 配合)
- Tauri onDragDropEvent 不给 DOM target
- HTML5 onDragEnter/onDragOver/onDragLeave 绑 auth-dir 按钮元素: enter → dragTargetRef="authdir"; leave → dragTargetRef="source"
- Tauri drop 读 dragTargetRef 决定路径去向
- **退化兜底**: 若 macOS WKWebView HTML5 onDragEnter 也不触发(与 onDrop 同病), dragTargetRef 恒 "source", auth-dir 拖拽无效 → auth-dir 回退 dialog(现有按钮保留)。源拖拽(主路径)纯 Tauri 不受影响
- 实现时先验 HTML5 onDragEnter 触发性: 挂载后 console.log 标记, 跑 dev 手动拖一次确认。不可靠则删 HTML5 分支, auth-dir 仅 dialog, prd 验收「auth-dir 拖入」条降级

### D. dragActive 视觉
- modal 根 div 动态边框: dragActive ? "2px dashed var(--accent)" : 现有
- 拖入时顶部浮一条提示条「📁 松开以导入 N 个文件」(dragActive && !parsing)
- 参 ImportExportTab dragActive 高亮模式

### E. parsing 状态协调
- 现有 setParsing 布尔; 拖入多源时任一 parse 在途 setParsing(true), 全部完成 false
- 用计数 ref(parseInFlight)避免并发多源时早期 false 覆盖
- dialog 单源 handleParse 保持原逻辑(也可复用 parseAndMerge)

## 改动文件
1. `src/components/platforms/CpaImportModal.tsx` — 主体: onDragDropEvent hook + handleDropSources + parseAndMerge 抽取 + dragActive 视觉 + auth-dir HTML5 target 识别(带退化)
2. `src/locales/*.json`(8 个) — 新 key: cpaImport.dropHint / dropActive / dragOverAuthDir 等

## 不改
- 后端 cpa_import.rs / cpa_import_parse 签名
- services/api/platforms.ts(cpaImportApi.parse 不变)
- apply 链

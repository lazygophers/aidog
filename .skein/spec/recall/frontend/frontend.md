---
title: frontend
category: frontend
keywords: [脏数据,浮点,归一,Number.isInteger,splitFraction,兼容性,tauri,drag,drop,wkwebview,html5,ondragdropevent,modal,state,architecture,PlatformEditForm,usePlatformForm,CpaImportModal,SmartPasteModal]
status: active
inclusion: auto
---

## 脏数据入库时归一 — 浮点 hour 拆分为整数 hour+minute

系统升级或跨版本迁移中，存量数据可能包含脏数据。例如，旧版本按整小时换算时产生 `start_hour: 8.5`（半时区换算残留），新版本期望整数。前端 parse 层应负责吸收这类脏数据。

## 陷阱：后端 migration 改 serde 类型成本高，数据污染持久

旧版本：`peak_hours` 整小时换算，半时区用户产生 `start_hour: 8.5` 写入 JSON。后端声明 `start_hour: i32`，JSON 反序列化失败 → 静默丢弃窗口。

修复选项：
- ❌ 改后端存储类型为 f64 —— 一个不再产生的格式永久兼容，代价不成比例
- ✅ **正解**：前端 parse 层吸收 —— 加载时拆分，用户下次保存自动正规化

## 前端读取路径归一（关键）

### MUST 单点归一（parse 层）

```ts
/** 存量非整数 start_hour/end_hour（半时区旧逻辑产物）拆为 hour+minute。
 *  整数值原样不动。 */
export function normalizeWindow(w: PeakWindow): PeakWindow {
  let result = w;
  if (!Number.isInteger(w.start_hour)) {
    const { hour, minute } = splitFraction(w.start_hour, w.start_minute);
    result = { ...result, start_hour: hour, start_minute: minute };
  }
  if (!Number.isInteger(w.end_hour)) {
    const { hour, minute } = splitFraction(w.end_hour, w.end_minute);
    result = { ...result, end_hour: hour, end_minute: minute };
  }
  return result;
}

/** 纯函数：浮点时刻拆为整数 hour+minute。 */
function splitFraction(h: number, existingMinute: number | undefined): { hour: number; minute: number } {
  const hour = Math.floor(h);
  const extraMinutes = Math.round((h - hour) * 60);
  return shiftClock(hour, (existingMinute ?? 0) + extraMinutes, 0);
}
```

### MUST 调用路径（两处 parse 出口）

```ts
// src/services/api/platforms.ts:204-216
export function parsePlatformPeakHours(raw: unknown): PeakWindow[] {
  // ... schema 校验 ...
  return windows.map(normalizeWindow);  // ← 追加归一
}

// src/services/api/platforms.ts:272
export function parsePlatformTimeModels(raw: unknown): TimeModelRule[] {
  // ... 循环处理 rule ...
  return rules.map(r => ({
    ...r,
    windows: r.windows.map(normalizeWindow),  // ← 追加归一
  }));
}
```

## 单测覆盖（脏数据拆分规则）

- 整数 hour 原样不变
- 8.0 视为整数不变
- 8.5 拆分为 8:30
- 已有 start_minute 时叠加进位
- start 与 end 各自独立归一

## 反例 / 常见错误

| 错误                          | 为什么错                                        |
| ----------------------------- | ----------------------------------------------- |
| 后端改 serde 类型为 f64        | 永久兼容一个不再产生的格式，代价不成比例        |
| 直接 Math.floor(h)，丢失分钟 | 8.5 → 8，漏掉 30 分钟，显示错误               |
| 判据用 `h % 1 === 0`         | 浮点舍入误差覆盖不全    |
| 忘记叠加已有 start_minute   | 8.5 + 已有 40m → 应是 9:10，但只变成 8:30    |

## 适用

- 版本升级中的数据兼容性问题
- 存量脏数据前端吸收而非后端永久兼容

## 关联

[[time-zone-minute-arithmetic]]、[[modal-state-architecture]]

## Tauri 拖拽事件 API（macOS WKWebView 限制）

Tauri 前端实现文件拖拽导入时，必须使用 Tauri `onDragDropEvent`，禁用 HTML5 onDrop（macOS WKWebView 不支持）。

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

[[modal-state-architecture]]

## PlatformEditForm Modal 架构模式

PlatformEditForm Modal 架构需要区分两类：直接灌表单 Modal 与跨表单 Modal，state 位置与传递方式完全不同。

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

新 Modal (如 Sub2Api)
├─ onApply 直接填表单字段？
│  └─ 是 → SmartPasteModal 模式（state 在 hook + PlatformPasteCtx）
└─ 否（返回中间数据由父级决策）？
   └─ 是 → CpaImportModal 模式（state 在组件本地 + onApplied 关闭）

## 验收

- [ ] grep `showCpaImport` / `showPaste` 在 PlatformEditForm 组件本地定义
- [ ] grep `PlatformPasteCtx` 不含跨表单 modal 的 state setter
- [ ] 跨表单 modal 的 `onApplied` 回调包含关闭逻辑

## 关联

[[tauri-drag-drop-api]]、[[form-level-tz-state-sharing]]

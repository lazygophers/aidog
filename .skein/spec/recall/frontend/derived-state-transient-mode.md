---
title: derived-state-transient-mode
name: derived-state-transient-mode
description: 派生态无法表现「空但已进入」中间态 — UI-only state 取代数据侧哨兵值
layer: recall
keywords: [派生态,中间态,UI-state,modal,选中态]
created: 1785564340
inclusion: auto
---

## derived-state-transient-mode

## 派生态无法表现「空但已进入」中间态 — UI-only state 取代数据侧哨兵值

当 UI 需要表现「已进入某模式但内容为空」的中间态时，从数据反推的派生态会失效（因为「空但已进入」与「未进入」在数据层同形）。正解是在 UI 层维护显式的选中态，而非向数据里塞空数组/哨兵值。

## 陷阱：派生态自我抵消

时段编辑 modal（`WindowsEditModal.tsx`）维度选择场景：

- 用户点「周几」，互斥逻辑清空月字段，周字段因尚未勾选任何一天而保持 `undefined`
- 此刻数据状态 = `{days_of_week: undefined, days_of_month: undefined}`
- 下一次渲染时 `dimensionOf(w)` 反推：无周数据 → 无月数据 → 返回「无」
- radio 选中态立刻弹回「无」

**问题根源**：「周几-但一天都没选」与「每天」在数据层不可区分，而 UI 需要它们可区分（前者需露出选择器）。

❌ 不该这样做：往数据塞空数组当哨兵
```json
{
  "days_of_week": [],
  "days_of_month": undefined
}
```
污染落盘数据与后端语义。

## MUST 在 modal/form 内维护 UI-only 的显式态

✅ **弹窗内单独维护 per-item 的维度选中态，渲染时优先取它**

```tsx
// 弹窗打开时初始化
const [uiDim, setUiDim] = useState<("none" | "dow" | "dom")[]>(
  windows.map(w => dimensionOf(w))  // 用数据侧推导初值
);

// 切换时同时更新 UI 态与数据
function switchDimension(idx: number, newDim: "none" | "dow" | "dom") {
  setUiDim(prev => {
    const next = [...prev];
    next[idx] = newDim;
    return next;
  });
  // 同时更新数据（互斥清空逻辑保持）
  updateWindow(idx, { days_of_week: newDim === "dow" ? [] : undefined, ... });
}

// 渲染时：优先取 UI 态，缺省回落数据反推
const currentDim = uiDim[idx] ?? dimensionOf(w);
```

## 坑点：按索引存的结构在删除中间项时会错位

若按 `uiDim[idx]` 存，删除中间窗口时必须同步搬移后续索引：

```tsx
// ❌ 删除后 uiDim 索引错位
windows.splice(idx, 1);
// uiDim[idx+1] 的状态会被错配到新的 windows[idx]

// ✅ 删除时同步过滤 uiDim
setUiDim(prev => prev.filter((_, i) => i !== idx));
windows.splice(idx, 1);
```

或改用稳定 key（object-based 结构）避免索引问题。

## 验收

- [ ] 中间态 state 在弹窗/表单组件内维护，不外泄到落盘数据
- [ ] 渲染表达式走 `uiState ?? deriveFrom(data)` 兜底模式
- [ ] 数据层零新增哨兵值或空数组标记
- [ ] 删除/增加列表项时同步维护 UI state 索引（若按索引存）或用稳定 key

## 适用

- Modal/form 内的选中态管理（非 URL 或持久化状态）
- 任何「空但已进入」的中间态（如选中分类但无具体项、进入编辑模式但内容空白）

## 关联

[[modal-state-architecture]]、[[form-level-tz-state-sharing]]

实例：peak-window-dimension-fix task 的 `WindowsEditModal.tsx` 修复

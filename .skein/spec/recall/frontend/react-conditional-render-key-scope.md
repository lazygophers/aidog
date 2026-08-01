---
title: react-conditional-render-key-scope
layer: recall
category: frontend
keywords: [react,performance,key,mount,lifecycle,animation]
source: src/App.tsx:198
authored-by: skein-spec
created: 1722470400
status: active
related: []
updated: 1722470400
---

## 触发场景
互斥条件渲染多个不同组件类型（`{cond && <ComponentA/>}`）时，发现容器设置了 key。

## 陷阱
误认为删除容器 key 会省去内部组件重新取数（re-fetch）。实际上：
- React 在互斥分支渲染**不同类型**组件时，本就做完整 mount/unmount
- **容器 key 只影响容器自身生命周期**，不代表控制内部组件生命周期
- 如果内部组件设计会在 mount 时取数，删 key 不会优化开销

## 正解
判断 key 目的：

| key 的实际用途 | 是否能删 | 原因 |
|---|---|---|
| 驱动 CSS animation 重放 | 否 | key 变化 → React 重挂容器 → CSS 重播 fadeIn |
| 强制状态重置 | 否 | key 变化清空容器内部 state（如 form input） |
| 列表去重 | 否（仅 `map` 用） | 列表 item 唯一标识，防止 move 混乱 |
| "我希望重取数据" | 是 | **应删 key**，改用 useEffect 依赖数组 + 显式重新取数 |

## 检查清单
```typescript
// 问诊流程：
// 1. 容器的 key 是否随条件变化？
<div key={effectiveNav}>
  {effectiveNav === "home" && <Home />}
  {effectiveNav === "settings" && <Settings />}
</div>

// 2. 删 key 后，是否内部组件重取数变多（而非减少）？
// 答：不变！因为 Home 和 Settings 本就不同组件，React 已完整 mount/unmount

// 3. key 是否绑定了 animation trigger？
// 在 CSS 里查 @keyframes 使用该类名
.animate-fade-in {
  animation: fadeIn 0.3s ease-in;  // ← 如果有，key 驱动重放，不能删
}
```

## 案例
`src/App.tsx:198` 的 `key={effectiveNav}`：
```tsx
<Suspense fallback={null}>
  <div className="animate-fade-in" key={effectiveNav}>
    {effectiveNav === "home" && <Home onNavigate={handleNavigate} />}
    {effectiveNav === "settings" && <AppSettings ... />}
    {/* 其他互斥分支 */}
  </div>
</Suspense>
```

- `effectiveNav` 变化 → div key 变化 → React 重挂容器
- CSS `fadeIn` animation 重播，视觉上页面淡入过度
- **不是为了省取数**，而是**为了重播动画**

## 适用
- 路由导航场景（页面切换动画）
- modal/popup 显隐（进入/离开 animation）
- 多 tab 页面（手动确认是否真需要 key 驱动重放）

## 非适用
- 某个 tab 内数据需重新取 → 改用 useEffect dependency
- 列表条目顺序变化 → 保持 `map((item) => <Item key={item.id} />)` 模式，不动容器 key

## 副作用
- 删错 key 不会省开销（内部组件 re-mount 开销不变）
- 删 key 会失去动画效果（用户察觉页面切换更生硬）

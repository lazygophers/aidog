---
title: shadcn 迁移测试改行为断言
layer: recall
category: test
keywords: [shadcn,测试,snapshot,行为断言,className]
source: -
authored-by: skein-spec
created: 1784730640
status: active
related: []
updated: 1784730640
---

## 触发场景
shadcn 迁移导致组件 className/结构变化，现有 snapshot 测试会因视觉差异失败。

## MUST 硬约束
测试改测行为而非 className；shadcn 迁移后 snapshot 应改为行为断言。

## 迁移模式
```tsx
// ❌ 旧：测试 className（脆弱）
expect(screen.getByTestId("card")).toHaveClass("bg-white");

// ✅ 新：测试行为（稳定）
expect(screen.getByText("Save")).toBeEnabled();
fireEvent.click(screen.getByText("Save"));
await waitFor(() => expect(onSave).toHaveBeenCalled());
```

## 适用
- PlatformCard/BalanceBar 等组件测试
- shadcn 迁移导致 className/结构变化的场景

## 关联
[[shadcn-select-none-sentinel]]

## 案例
- shadcn-pages task：PlatformCard.test.tsx snapshot → 行为断言（删除 2096 行快照）

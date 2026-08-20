---
title: component-test-structure-assert
name: component-test-structure-assert
description: 组件测试按结构断言而非文案 — 并发改动时文案断言必然破碎
layer: recall
keywords: [test,组件,文案,结构,断言,i18n,并发改动]
created: 1785564340
inclusion: auto
---

## component-test-structure-assert

## 组件测试按结构断言而非文案 — 并发改动时文案断言必然破碎

组件测试禁止按文案文字选择元素或断言输出内容（如 `getByText("周几")`），改为按元素结构（id / data-state / title 属性 / 回调入参对象结构）。理由不只是「文案会改」，而是**并发改动时按文案断言的测试必然被打碎**。

## 陷阱：getByText 在文案改动时脆弱

WindowsEditModal 组件测试（6 个用例）：测试按文案选择和断言：

```tsx
// ❌ 反例：按文案文字选择 / 断言
fireEvent.click(screen.getByText("周几"));
expect(screen.getByText("周几")).toBeInTheDocument();
expect(screen.getByText("每月几日")).toBeVisible();
```

同时有多个 agent 改动：
- test agent 跑这 6 个用例
- i18n agent 改 8 个 locale 的 `windows_dimension` 和 `windows_dim_none` key 值

**碰撞**：
1. test 运行到第 2 个用例，fireEvent.click 查 getByText("周几")
2. 同时 i18n agent 把 `windows_dim_none` 从「无」改为「每天」，触发 locale 文件改动
3. getByText 匹配失败，test 炸

## MUST 按元素属性或回调入参结构断言

✅ **结构断言方式**（React Testing Library）

```tsx
// 按 id / data-* 属性选择
const weekButton = screen.getByRole("radio", { name: /无|every|каждый/i });
fireEvent.click(weekButton);

// 按元素存在性与可见性（不涉及文案）
expect(screen.getByTestId("dow-selector")).toBeVisible();
expect(screen.queryByTestId("dom-selector")).not.toBeInTheDocument();

// 按回调入参的对象结构
expect(mockOnSave).toHaveBeenCalledWith(
  expect.objectContaining({
    windows: expect.arrayContaining([
      expect.objectContaining({
        days_of_week: expect.any(Array),
        days_of_month: undefined,
      }),
    ]),
  })
);

// 按 HTML 属性（而非文案内容）
const radio = screen.getByRole("radio");
expect(radio).toHaveAttribute("data-state", "dow");
expect(radio).toHaveAttribute("title", expect.stringMatching(/day|jour|день/));
```

## 断言接缝分类

| 接缝 | 风险等级 | 说明 |
|---|---|---|
| `getByText()` / `getByRole(..., {name: /text/})` | 🔴 高 | 文案改动时失效，i18n 并发改动必然碰撞 |
| `getByTestId()` / `data-testid` 属性 | 🟢 低 | 结构标记，独立于文案 |
| `toHaveAttribute()` / `title` 属性 | 🟡 中 | 需保证属性名稳定，但改动频率低 |
| 回调入参对象结构 | 🟢 低 | 数据契约，与 UI 文案完全解耦 |
| `querySelector()` 选择类名/ID | 🟡 中 | 类名改动时失效，但 CSS refactor 频率低 |

## 验收

- [ ] 全部 getByText() 替换为 getByTestId() 或 getByRole() + 结构匹配
- [ ] 回调（如 onSave / onChange）入参用 `toHaveBeenCalledWith(expect.objectContaining(...))`
- [ ] 若需判定元素可见性，用 `toBeVisible()` / `toBeInTheDocument()` 而非查其中文案
- [ ] 多语言组件（含 i18n key）的测试中，禁出现任何母语文案（如「周几」「เสาร์」「Samstag」）

## 适用

- React 组件单元测试（React Testing Library / Vitest）
- 任何涉及 i18n 的组件（8+ 语言时 i18n agent 常与测试并发）
- UI 选中态、展开/收起、错误提示等行为类测试

## 实例

WindowsEditModal 组件测试（6 用例）：按结构断言（getByTestId / toHaveBeenCalledWith）而非文案，避免与 i18n agent 并发改动时测试失败。peak-window-dimension-fix task 完成。

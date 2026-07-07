# 浮窗自动隐藏

## Goal

让 aidog 浮窗（popover）按"自动隐藏"行为收起。**结论：现有失焦自动关闭已满足需求，无需新开发。**

## 结论 (ADR-lite)

**Context**: 用户提"浮窗视角自动隐藏"，澄清后发现是不知道现有功能已存在。
**Decision**: 不开发。现有失焦自动关闭（`popover.tsx:136-143`）即用户所需。
**Consequences**: 零代码改动。仅澄清 + 文档化触发方式。

## 现有失焦关闭链路 (已验证)

- 触发：托盘左键 Down → 弹出 popover（`focused(true)`，`app_setup.rs:282`）
- 关闭路径 1（失焦）：点 popover 外任何地方 → `onFocusChanged(!focused)` → `destroy()`（`popover.tsx:136-143`）
- 关闭路径 2（托盘 toggle）：popover 已开时再点托盘 → 主动 `destroy()`（`app_setup.rs:250-253`）
- 两路径去重：重复 destroy 由 `.catch(()=>{})` 吞已销毁错误

## Requirements

- 无新增（现有满足）

## Acceptance Criteria

- [x] 现有失焦关闭链路 trace 无 bug（代码静态验证）
- [x] 用户确认「现有即可」

## Out of Scope

- 定时隐藏 / 鼠标离开隐藏 / 靠边吸附（用户未要）

## Technical Notes

- 失焦关闭：`src/popover.tsx:136-143`
- 窗口创建：`src-tauri/src/app_setup.rs:242-289`
- 窗口属性：`always_on_top(true)` / `skip_taskbar(true)` / `focused(true)`

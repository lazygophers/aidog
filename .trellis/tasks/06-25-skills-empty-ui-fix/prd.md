# 修 skills 列表 UI 空显示 bug (skills-empty-ui-fix)

## 目标
修复：skills 实际不为空，但 UI 列表显示为空的 bug。

## 背景
- 用户反馈：skills 不为空（已安装有 skill / 后端能取到数据），但 UI 显示为空列表。
- 须先复现 + 定位根因，再最小修复。

## 排查方向 (Explore 须做, 勿臆断根因)
- 数据链路：后端 skills 列表 command（lib.rs）→ `services/api.ts` invoke 封装 → `Skills.tsx` 消费渲染。
- 高发根因候选（逐一排除，按记忆经验）：
  - Rust↔TS 边界字段名/类型错位致前端拿到 undefined/空（[[tauri-invoke-param-camelcase]] / boundary）。
  - mount fetch 晚到 resolve 覆盖 / StrictMode 双跑 / 状态守卫缺失（[[mount-fetch-late-resolve-overwrites-optimistic]]）。
  - 过滤/分组逻辑把全部项过滤掉（如按 agent/enable 过滤条件错误）。
  - 列表渲染条件（loading/empty 判断）逻辑错误。
  - 锁文件/source enable 解析（[[skills-management-module]]）取不到致列表判空。
- 用 console / 运行时探针确认数据到没到前端、在哪一层断。

## 需求拆解
- 复现 → 定位断点层 → 最小修复（改最小面，禁掩盖症状）。
- 修复后：skills 非空时正确渲染列表。

## 验收标准
- 已安装 skills 非空时 UI 正确显示全部 skill（复现场景修复）。
- 空时仍正确显示空态（不破坏空态）。
- `yarn build` 通过；若改后端 `cargo build/clippy/test` 绿；`check-i18n` 绿（若涉及文案）。
- 根因在交付说明里写清（哪一层断、为何、怎么修）。

## 非目标
- 不顺手重构 Skills 页其它逻辑，仅修空显示 bug。

## 单一交付
单 worktree。
## 串行依赖
skills 串行轨第 2 项：skills-search (1) → 本任务 (2) → skills-remove-grouping (3)。基线须含前序已合并改动。

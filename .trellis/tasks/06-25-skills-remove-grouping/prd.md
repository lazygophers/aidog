# 移除 skills 分组概念 (skills-remove-grouping)

## 目标
移除 Skills 模块的「分组」概念，简化为统一扁平列表。

## 背景
- 用户反馈：移除 skills 分组。
- 现状（须先核实）：Skills 已是「统一列表(不分 agent)」（[[skills-management-module]]），但可能仍残留某种分组维度（按 agent / source / category 分组的 UI 或数据结构）。须先确认「分组」具体指什么再移除。

## 排查 (Explore 须先确认「分组」指什么)
- Skills.tsx / SkillInstallView / SkillDetailView 中是否有分组 UI（分组标题/折叠区/tab/按某维聚合渲染）。
- 后端是否有分组相关字段/存储/command。
- 确认范围后移除分组渲染 + 相关 dead 数据结构/字段（前后端一致清理，禁留死代码）。

## 需求拆解
- 移除分组 UI，改为统一扁平列表渲染。
- 清理因移除分组而无用的状态/字段/类型/后端逻辑（前后端边界一致）。
- 保留 skill 本身的安装/启用/详情功能不变。

## 验收标准
- Skills 页无分组维度，统一扁平列表。
- 无残留死代码/未用字段/未用 import（cargo/tsc/lint warning 清零）。
- `yarn build` + `cargo build/clippy/test`(若涉后端) + `check-i18n` 全绿。
- 移除范围在交付说明写清（移除了哪些分组结构）。

## 非目标
- 不改 skill 安装/启用/详情核心功能。

## 单一交付
单 worktree。
## 串行依赖
skills 串行轨第 3 项（末项）：skills-search (1) → skills-empty-ui-fix (2) → 本任务 (3)。基线须含前两项已合并改动。

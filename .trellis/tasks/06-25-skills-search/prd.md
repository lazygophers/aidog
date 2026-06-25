# 已安装 skills 支持搜索 (skills-search)

## 目标
Skills 管理页 (`src/pages/Skills.tsx`) 的**已安装 skills 列表**支持搜索过滤：用户输入关键词，实时过滤出名称/描述匹配的 skill。

## 背景
- 用户反馈：已安装的 skills 需要支持搜索。
- 现状（须先核实）：Skills 页统一列表（不分 agent，见 [[skills-management-module]]），列表可能较长，缺搜索入口。

## 需求拆解 (前端单页)
- 列表上方加搜索框（与项目既有搜索框样式一致 —— 参照 Platforms/Groups 页搜索框，禁自创）。
- 实时过滤：匹配 skill name + description（如有 source/标签也纳入）。
- 支持中文拼音搜索若项目已有该能力（`src/utils/pinyin.ts`）—— 复用，不自造。
- 空搜索结果有友好提示（i18n）。
- 清空搜索恢复全量列表。

## 复用
- 搜索框组件/样式：Platforms.tsx / Groups.tsx 既有实现。
- 拼音搜索：`src/utils/pinyin.ts`（若适用）。

## 验收标准
- 已安装 skills 列表上方有搜索框，输入实时过滤。
- 名称/描述匹配生效；清空恢复全量；无结果有提示。
- `yarn build` + `node scripts/check-i18n.mjs`（新增搜索框 placeholder/无结果文案 8 locale 全覆盖）绿。

## 非目标
- 不改 skill 安装/卸载/启用逻辑。
- 不动后端。

## 单一交付
纯前端单页，单 worktree。
## 串行依赖
属 skills 串行轨：本任务 (1) → skills-empty-ui-fix (2) → skills-remove-grouping (3)。同改 Skills.tsx，须串行避免冲突。

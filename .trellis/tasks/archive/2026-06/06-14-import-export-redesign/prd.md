# 导入导出 UI 视觉重设（Liquid Glass 系统内）

## Goal

把 `src/components/settings/ImportExport.tsx`（AppSettings「导入导出」tab）从「裸 checkbox 网格 + 朴素按钮 + radio 冲突行 + 纯文本 report」提质为与 app 设计系统（Liquid Glass + 9-style 主题 + components/shared）一致的精品 UI。**功能不变**（同 API、同 scope、同冲突决策模型），纯展示层重设 + 少量 UX affordance（全选/批量冲突/分段控件）。

## What I already know（现状 + 设计系统锚点）

- 现状 `ImportExport.tsx`(316 行)：导出区(scope checkbox 2 列网格 + 朴素 btn-primary) + 导入区(选文件 btn → preview meta + counts 文本 + ConflictRow radio + apply) + ReportView(✓/⊘ 文本列表) + 内联 toast。消费 `importExportApi.exportToFile/readPreview/apply`。8 个 scope（platform/group/group_platform/setting/codex/claude_code/model_price/skills），各有 labelKey + 中文 defaultLabel。冲突决策 overwrite/skip/rename(+new_key)。
- **设计系统可复用件**（真实锚点）：
  - `SectionIcon({name,size})`（editors.tsx:96）图标名集：core/behavior/ui/team/permissions/env/hooks/plugins/sandbox/attribution/status/network/memory/worktree/advanced/folder/file/bolt。**禁 emoji**。
  - `StatChip({icon,value,label,color?,level?})`（shared/StatChip.tsx）—— preview counts 用。`level: success|warning|danger|neutral` → `--color-*`。
  - `CompactCard`（shared，可展开卡）—— 视情况用。
  - CSS 类 `.glass`/`.glass-surface`/`.glass-elevated`、`.btn-primary`。
  - 主题令牌（随 9-style 自适应，**必须用**）：`--radius-sm/md/lg`、`--shadow-sm/md`、`--transition`、`--accent`/`--accent-hover`/`--accent-subtle`、`--border`/`--border-focus`、`--bg-glass`/`--bg-surface`/`--bg-floating`、`--text-primary/secondary/tertiary`、语义色 `--color-success/-danger/-warning` + `--color-*-bg`（见 colorScale.ts，禁硬编码 rgba 主题色）。
  - 数值格式化走 `utils/formatters.ts`（如有计数展示）。
- scope→icon 映射（用现有图标，无 emoji）：platform→network、group→team、group_platform→worktree、setting→bolt、codex→file、claude_code→memory、model_price→advanced、skills→plugins。

## Decision (ADR-lite)

**Context**: 导入导出是 app 内少数没跟上 Liquid Glass 质感的页；功能稳定，问题纯在视觉/可读性。
**Decision**: 纯展示层重设，复用设计系统（SectionIcon/StatChip/glass 类/主题令牌/语义色），不碰 api 契约与决策数据模型。新增 affordance：scope 卡片化 + 全选/反选 + 选中计数、拖放式导入入口、冲突分段控件 + 批量覆盖/跳过、report 卡片化(语义色分区)。
**Consequences**: 主题切换(9 style × 12 palette)下自适应(令牌驱动)；i18n 增量新增几个 key；行数可能增至 ~450（仍单文件，可接受）。

## Requirements

### 重设计规格（逐区）

**A. 导出区**
- section 头：`SectionIcon name="folder"` + 标题「导出」 + 描述（沿用 exportDesc）。
- scope 选择 → **卡片化**（替代裸 checkbox 网格）：每 scope = 可点击 glass 卡（`.glass-surface`），含 `SectionIcon`(映射) + label(粗) + 一行描述(现 defaultLabel 的括号说明转描述) + 右上角选中指示(✓ 勾，自绘 SVG 或 SectionIcon，非 emoji)。**选中态** `border:1px solid var(--accent)` + `background:var(--accent-subtle)`；未选 `border:1px solid var(--border)` 透明底。整卡点击 toggle，hover 微抬(shadow/transition 令牌)。grid `repeat(auto-fill,minmax(220px,1fr))`。
- scope 区头一行：「导出范围」+ 「全选 / 反选」文字按钮 + 选中计数 chip（如「已选 4 / 8」）。
- 导出按钮：`.btn-primary`，文案带计数「导出 {n} 项」(n=0 禁用)；导出中「导出中…」。
- 成功 → **glass-elevated 成功卡**（替代裸 toast）：✓ + 文件路径(可截断/可复制)，`--color-success` 描边/图标。

**B. 导入区**
- section 头：`SectionIcon name="worktree"` + 标题「导入」 + 描述(沿用 importDesc)。
- 选文件 → **拖放式入口**（视觉拖放风，实际仍点击触发 Tauri open；不实现真 drag-drop）：虚线 glass 区(`border:1.5px dashed var(--border)`)，含 `SectionIcon name="file"` + 「选择 .aidogx 文件」+ 副文案「自动解密 · Skill 自动安装」。hover 边框转 `--accent`。
- preview → **概要卡**：来源机器 + 导出时间 作 meta 行(label+value)；counts 作 **StatChip 行**（每 scope 一个 chip：映射 icon + 数值 + scope label）。
- 冲突 → 清晰行：每条 `.glass-surface` 行含 `[scope] key`(粗) + existing_summary(次级) + **分段控件**(覆盖/跳过/重命名，替代 radio：3 段按钮组，选中段 `--accent-subtle`+`--accent` 文字)；选「重命名」→ 行内 input 出现(默认 key+"-imported")。冲突组头「冲突 {n} 项」+ 「全部覆盖 / 全部跳过」批量按钮(一次性 setDecisions)。
- 应用按钮：`.btn-primary`「应用导入」/「导入中…」。
- report → **结果卡**：applied(✓ `--color-success` 区) / skipped(⊘ `--text-tertiary` 中性区) / errors(`--color-danger` 区，有则显)。每类用小标题 + 行；可用 StatChip 概括数量。

**C. 通用**
- 全部 radius/shadow/transition/色 走主题令牌(随 9 style 自适应)，禁硬编码 hex/rgba 主题色（语义色走 `--color-*`，见 [[theme-css-var-names]] / colorScale.ts）。
- 错误 toast 保留但样式对齐(用 `--color-danger`/`--color-danger-bg`)。
- **功能/数据流 100% 不变**：scopes Set、decisions Map、handleExport/handlePickFile/handleApply、api 调用全保留，只改 JSX/样式/拆子组件。
- i18n 8 locale 新增：`importExport.selectAll`(全选) / `deselectAll`(反选) / `selectedCount`(已选 {{n}}/{{total}}) / `exportN`(导出 {{n}} 项) / `dropHint`(拖放副文案) / `bulkOverwrite`(全部覆盖) / `bulkSkip`(全部跳过) 等所需 key（沿用已有 key 不动）。

## Acceptance Criteria

- [ ] scope 卡片化：8 卡各带映射图标 + 描述 + 选中态(accent 边+subtle 底+✓)，整卡可点 toggle，全选/反选/计数可用。
- [ ] 导出按钮显选中计数、0 项禁用；成功显路径卡。
- [ ] 导入拖放式入口；preview 用 StatChip 显 counts + meta 行。
- [ ] 冲突分段控件(覆盖/跳过/重命名)替代 radio，重命名行内 input；批量覆盖/跳过可用。
- [ ] report 卡片化，applied/skipped/errors 语义色分区。
- [ ] 9 style × 明暗下视觉自适应(令牌驱动)，无硬编码主题色塌陷；无 emoji。
- [ ] 功能回归：导出/选文件/预览/冲突决策/应用全链路与改前等价（api 调用、决策模型不变）。
- [ ] `yarn build` + `yarn check:i18n`(新 key 全 8 locale) 通过，无 tsc warning，无 any。

## Out of Scope

- 后端 / api 契约 / 决策数据模型变更。
- 真实 drag-drop 文件(仅视觉拖放风 + 点击触发)。
- 新增 scope / 新功能。
- 其它设置页 UI。

## Technical Notes

- code-reuse：复用 SectionIcon/StatChip/glass 类/formatters，禁另造图标或重复 chip。读 `.trellis/spec/guides/code-reuse-rules.md`。
- frontend/conventions：组件/状态/类型/i18n/无 any。子组件(ScopeCard/Segmented/DropZone/PreviewCard/ConflictRow/ReportCard)可拆但留同文件或 shared，按现有 ImportExport 单文件惯例。
- 主题令牌单一事实源：禁 `rgba(255,255,255,…)` fallback、禁裸 hex 主题色（见 [[theme-css-var-names]]）。
- 反 AI slop：无紫渐变、无 emoji 图标、诚实边界、令牌色不凭空发明。
- 验证手动：dev → 设置/导入导出 → 切几个主题(liquidGlass/sketchy/aurora)看自适应 → 走一遍导出 + 导入预览。

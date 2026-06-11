# 重设计 Claude Code 设置页（Settings.tsx）

## Goal

全维度重设计 aidog 的 Claude Code 设置页（`src/pages/Settings.tsx`，编辑 `~/.claude/settings.json`）——布局、UI/UX、人机交互、配色。从 VS Code 侧栏范式改为单页分区滚动设置面板，精修 Liquid Glass 视觉，升级保存交互，并顺手把 4680 行单文件拆成可维护组件。功能零回退。

## What I already know（现状测绘）

- **布局**：页头 GUI/JSON 模式切换 + 左 220px 固宽搜索侧栏 + 右内容窗格（VS Code 风格）。
- **10 个 section**：core(23) / ui(10) / permissions / env(结构化) / hooks / plugins / sandbox / status(3) / worktree(1) / advanced(10)。
- **字段类型**：boolean(Toggle) / select / json(JsonEditor) / kv(KvEditor) / string[](StringListEditor) / string(+pathType 拾取)。
- **特殊编辑器**：EnvEditor（分组+搜索）、PermissionsSection（allow/ask/deny 递归，纯 JSON 子编辑）、PluginsSection（SortableList）、HooksSection、SandboxSection、StatusLineSection。
- **机制**：手动 Save、从 Claude Code 导入（diff 树合并）、加载推荐配置（RECOMMENDED_CONFIG）、侧栏全局搜索。
- **视觉**：Liquid Glass 主题，全走 CSS 变量（无硬编码色），`.glass*` 类、设计令牌 F/S（字号/间距，Settings.tsx:18-36）。蓝 accent #007AFF/#4A9EFF。
- **i18n**：section/field label、env 走 t()；部分 field description 与权限内部文案英文硬编码。
- **痛点**：单文件 4680 行、label|control 行布局重复、EnvVarRow(220px)≠FieldLabel(200px)、权限无可视化、部分文案未 i18n。

## Decisions（ADR-lite）

| 维度 | 决策 |
|------|------|
| 布局范式 | **单页分区纵向滚动** + 顶部粘性 section 锚点 chip 快跳；取消左固宽侧栏 |
| 配色/视觉 | **精修 Liquid Glass**，沿用蓝 accent 与现有主题变量；提升层次（留白/柔卡片/细折射边缘/阴影） |
| 保存机制 | **手动保存 + 升级提示**：未保存脏状态提示条 + 切页/离开拦截 + Cmd+S 快捷键 |
| 架构 | **拆分单文件** 到 `src/components/settings/`，统一 SettingRow 行布局 |
| 增项 | 全部纳入：① 保留全局搜索（移到顶栏）② 权限可视化编辑器 ③ 字段级「恢复默认」④ 粘性顶栏操作区 |

**Consequences**：工作量较大、回归面广（双模式/导入/推荐/搜索/各特殊编辑器需逐一验证不回退）；收益是可维护性 + UX + 视觉一次性升级。

## Requirements

### 布局
- R1 单页纵向滚动承载全部 section，移除左 220px 固宽侧栏。
- R2 **粘性顶栏**：模式切换(GUI/JSON) + 全局搜索框 + 导入 + 推荐 + 保存按钮 + 未保存脏状态指示，滚动时常驻。
- R3 **section 锚点导航**：顶栏下方粘性 chip 行（各 section 图标+名），点击平滑滚动到对应 section，滚动时高亮当前 section（scroll-spy）。
- R4 每个 section 渲染为 Liquid Glass 分组卡片，统一标题（图标+i18n label）+ 字段列表。

### 视觉
- R5 沿用现有主题 CSS 变量与蓝 accent，明暗双模式正常；精修间距/卡片/边缘/阴影层次。
- R6 统一 `SettingRow`：label|control 对称行布局，消除 220/200 宽度不一致。

### 交互
- R7 手动保存保留；新增未保存「脏」状态 → 顶栏提示 + 离开/切页拦截 + `Cmd/Ctrl+S` 触发保存。
- R8 **全局搜索**移到顶栏：查 section/field label/description，命中高亮 + 滚动定位，未命中 section 淡出/隐藏。
- R9 **权限可视化编辑器**：allow/ask/deny 规则列表可视增删 + defaultMode 切换，替代裸 JSON（保留 JSON 兜底入口可选）。
- R10 **字段级「恢复默认」**：被改动且有默认值的字段显示标记，悬停/点击可重置为默认。

### 架构
- R11 从 `Settings.tsx` 抽出到 `src/components/settings/`：`SettingRow`、`FieldRenderer`、`EnvEditor`、`PermissionsEditor`、`PluginsSection`、`HooksSection`、`SandboxSection`、`StatusLineSection`、`AnchorNav`、`SettingsHeader`。主文件降为编排容器。

### 不回退
- R12 双模式(GUI/JSON)、导入、推荐配置、各特殊编辑器、env 分组搜索全部保留可用。
- R13 新增文案走 t()，7 语言不破坏；顺手补齐权限/字段 description 的 i18n（尽力，不阻塞）。

## Acceptance Criteria

- [ ] 单页滚动 + 顶部粘性锚点导航，点击平滑滚动、scroll-spy 高亮当前 section。
- [ ] 顶栏粘性，含模式切换/搜索/导入/推荐/保存/脏状态，滚动常驻。
- [ ] 改字段后顶栏显「未保存」，切页/关页有拦截，Cmd/Ctrl+S 保存生效。
- [ ] 全局搜索可过滤定位字段，命中高亮。
- [ ] 权限可视化增删 allow/ask/deny + defaultMode 切换，写回 settings 正确。
- [ ] 改动字段可一键恢复默认。
- [ ] GUI/JSON 切换、从 Claude Code 导入、加载推荐配置、env 分组搜索全部正常无回退。
- [ ] 明暗双主题 + 7 语言（含 ar-SA RTL）渲染正常。
- [ ] Settings.tsx 拆分到 `src/components/settings/`，主文件显著瘦身。
- [ ] typecheck / lint green。

## Definition of Done

- 功能不回退（双模式/导入/推荐/搜索/各特殊编辑器）。
- Tauri command 契约不变（settingsApi / claudeSettingsImportApi / statuslineApi 字段/类型/返回值）。
- i18n 7 语言不破坏，新增文案走 t()。
- typecheck / lint green；明暗双模式正常。
- 视觉沿用 Liquid Glass 主题变量。

## Out of Scope

- 不改 settings.json schema 本身（字段集不增删）。
- 不改后端 Rust Tauri command。
- 不引入新主题引擎 / 不换设计系统。
- 不改 AppSettings.tsx 等其他页面。

## Technical Notes

- 主文件：`src/pages/Settings.tsx`(4680)、`src/services/claude-settings-schema.ts`(403)、`src/themes/liquidGlass.ts`、`src/styles/globals.css`。
- 设计令牌：Settings.tsx:18-36（F 字号 / S 间距）。
- 拆分目标目录：`src/components/settings/`（新建）。
- scroll-spy 用 IntersectionObserver；锚点平滑滚动 `scrollIntoView({behavior:'smooth'})`。
- 离页拦截：路由层 + `beforeunload`（Tauri 窗口）。

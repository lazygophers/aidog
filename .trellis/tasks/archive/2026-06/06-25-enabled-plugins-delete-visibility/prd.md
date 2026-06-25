# Enabled Plugins 删除按钮可见性 (enabled-plugins-delete-visibility)

## 目标
让 Settings → Plugins 的 **Enabled Plugins** 列表每项的删除控件清晰可识别，用户能一眼认出"删除"。

## 背景 / 根因 (已核实)
- 用户反馈："插件(Enabled Plugins)应该支持删除"，澄清后：**只看到启用/关闭(Toggle)，没看到删除按钮**。
- 代码核实：删除功能**已存在**且生效 —— `src/components/settings/editors.tsx` 的 `PluginsEditor`：
  - `removePlugin` (editors.tsx:3001-3005) 逻辑正确（delete key → updateField，空则 undefined）。
  - 渲染处 (editors.tsx:3049-3052) 每行 = `<code>{key}</code>` + `<Toggle>` + `<button onClick={removePlugin}><IconClose size={12}/></button>`。
  - 删除按钮样式 `color: var(--text-tertiary)` + `IconClose size={12}`，紧贴 Toggle，**太淡太小，被误认为装饰/认不出** → 纯 UX 缺陷，非功能缺失。
- 两处渲染路径（`PluginsSection` 3128 / `PluginsSectionInline` 3145）都走同一个 `PluginsEditor`，故只需改 `PluginsEditor` 内的渲染。
- **Extra Marketplaces** 删除按钮 (editors.tsx:3089-3092) 是**完全相同的 faint 样式**，存在同样问题。

## 方案 (用户已拍板)
醒目**垃圾桶(trash)图标** + **危险色悬停**：
- 图标：垃圾桶 SVG（非 ×/IconClose），尺寸 14-16px。
- 默认色：中性（`var(--text-tertiary)` 或 `--text-secondary`，比当前略明显）。
- 悬停：变红（danger 色，项目若有 `--danger`/`--error` CSS 变量则用，无则 `--accent` 或合适红色变量，**禁硬编码 rgba**，遵 [[theme-css-var-names]]）。
- 与 Toggle 拉开间距（避免误触/视觉粘连）。
- **同步修 Extra Marketplaces 删除按钮**，保持两处一致。

## 排查 / 复用 (Explore 先做)
- 确认项目是否已有垃圾桶图标组件/路径（grep `ICON_PATHS`、`SvgIcon`、现有 trash/delete d 路径）—— 有则复用，无则按现有 `SvgIcon`/`ICON_PATHS` 模式新增一个 trash path，禁自造一次性内联。
- 确认 danger/红色 CSS 变量名（查 `src/themes/` 各主题变量），禁硬编码颜色、禁 `rgba(255,255,255)` fallback。
- 悬停变色：用 React state(hover) 或 CSS class，与项目既有 hover 交互模式一致。

## 需求拆解
- 替换 `PluginsEditor` 内 Enabled Plugins 行的删除按钮：IconClose(12px,tertiary) → trash 图标(14-16px) + hover danger。
- 同步替换 Extra Marketplaces 行删除按钮为同样样式。
- 删除按钮加 `title`/`aria-label`（i18n，如 `settings.plugins.removePlugin`）提升可达性 + tooltip 提示。
- 与 Toggle 间距拉开。

## 验收标准
- Enabled Plugins 每项删除按钮为可识别的垃圾桶图标，悬停变红，用户一眼认出。
- Extra Marketplaces 删除按钮同步一致。
- 删除功能行为不变（仍调 `removePlugin`/`removeMarketplace`）。
- 颜色走 CSS 变量，无硬编码 rgba，无 `rgba(255,255,255)` fallback（[[theme-css-var-names]] / [[floating-bg-variable]]）。
- 新增 i18n 文案（title/aria-label）8 locale 全覆盖。
- `yarn build` + `node scripts/check-i18n.mjs` 全绿，无 tsc warning。

## 非目标
- 不改删除的业务逻辑（removePlugin/removeMarketplace 不动）。
- 不重构 PluginsEditor 其它部分。
- 不动后端。

## 单一交付
纯前端单页(editors.tsx)，单 worktree。

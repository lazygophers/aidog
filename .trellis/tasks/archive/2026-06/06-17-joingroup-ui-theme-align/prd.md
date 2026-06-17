# PRD: 加入已有分组 UI 对齐主题设计

## 现状（问题）

上任务 `platform-add-group-option` 交付的「分组归属」表单段 UI 未对齐代码库设计系统（globals.css）：

- **「创建默认分组」**用裸 `<input type="checkbox">` + 文本。代码库 boolean 选项走 `.toggle-wrap` + `.toggle` 开关（如 ImportExport 备份 `backup.enable`）。
- **「加入已有分组」chips** 自造 pill：`borderRadius:999 / fontSize:12 / 无 fontWeight / 1px border / 内嵌原生 checkbox`。设计系统的 `.badge` 是 `radius:6 / fontSize:11 / fontWeight:600 / 无 border 用 bg / 无 checkbox`。差异明显。

## 目标（用户已定方向：保留 pill chips 仅调参）

1. boolean（创建默认分组 / batchAutoGroup）→ `.toggle-wrap` + 隐藏 checkbox + `.toggle` span 开关（同备份启用开关）。
2. 多选 chips（加入已有分组 / batchJoinGroupIds）→ **保留 pill 形（radius 999）**，但：
   - 去掉内嵌原生 checkbox（点 pill 切换选中态）。
   - 全走 CSS 变量（选中 `--accent-subtle` bg + `--accent` 文字/边；未选 `--bg-glass` bg + `--text-secondary`）。
   - 加 hover / transition + cursor:pointer，对齐主题质感。
   - 排版微调（fontSize 12 / 适中 padding）使其与表单其他控件协调。

## 设计系统依据（globals.css）

- `.toggle`(L285)：40×22，`--bg-glass` bg / `--border` border，`.active`→`--accent`；`.toggle-wrap` 无 CSS 规则（hook-only，内联 cursor/flex）。
- `.badge`(L321)：`padding:2px 8px / radius:6 / fontSize:11 / fontWeight:600`；`.badge-accent`(accent-subtle bg+accent text)、`.badge-muted`(bg-glass+text-secondary)。
- 既有用法：Platforms.tsx(L1465/2756/2838)、Logs.tsx(L582) 大量 `badge badge-accent/muted`。

## 改动范围

- `src/pages/Platforms.tsx`：分组归属 FormSection（autoGroup→toggle；join chips→badge）。
- `src/components/settings/CcSwitchImport.tsx`：批量分组选择器（batchAutoGroup→toggle；join chips→badge）。
- 不动后端、不动逻辑、不加 i18n key（仅样式）。

## 验收

1. `yarn tauri dev` → 添加/编辑平台表单 + cc-switch 导入的分组区：开关 + 徽章 chips 视觉与其他设置项（备份开关、协议徽章）一致。
2. `npx tsc --noEmit` 0 error。
3. 选中/未选 chips 切换正常（badge-accent ↔ badge-muted）。
4. 暗色/亮色主题 + 现用调色板下无违和（走 CSS 变量，主题自适应）。

## subtask

单一交付（2 文件样式对齐），不拆 child。

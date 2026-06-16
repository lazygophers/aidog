# 导入导出 UI 适配主题

## 根因（审计 ImportExport.tsx）

1. **CSS 变量名错**：用 `--color-success` / `--color-danger` / `--color-border`，但主题实际变量是 `--border`（非 `--color-border`），且**无 success/danger 语义色变量** → 全走 fallback 硬编码 hex（`#2ea04360`/`#da363360`/`#f85149`/`#333`），不适配主题 dark/light/调色板。
2. **toast 内联 background 覆盖**：`.toast` CSS 本用 `var(--bg-elevated)`（主题适配），但 ImportExport 内联 `style={{ background: var(--color-success,硬编码) }}` 覆盖 → 强制硬编码色。
3. ConflictRow border `var(--color-border, #333)` / ReportView error 文字 `var(--color-danger, #f85149)` 同病。

## 修复

### 1. 主题加语义色变量（6 主题 × light/dark）

新增 `--success` / `--danger`（纯色，按各主题调色板）：
- liquidGlass: `#34C759` / `#FF3B30`（iOS 系统色，light/dark 同）
- nord: `#a3be8c` / `#bf616a`（nord14/nord11）
- dracula: `#50fa7b` / `#ff5555`
- catppuccin: light `#40a02b`/`#d20f39`，dark `#a6e3a1`/`#f38ba8`
- solarized: `#859900` / `#dc322f`

### 2. ImportExport.tsx 修变量名 + toast 语义化

- toast 成功：`background: var(--bg-elevated); border-color: var(--success); color: var(--success)`（背景用主题卡色，border+文字传语义）
- toast 错误：同上用 `--danger`
- ConflictRow border：`var(--border)`（去 fallback）
- ReportView error 文字：`var(--danger)`
- 去所有 `--color-*` + 硬编码 fallback

### 3. 不做

- 不改 `.btn-danger` 全局 CSS 硬编码（范围蔓延，另开 task）
- 不改 checkbox/radio 原生控件样式（原生 + glass 容器已够）

## Acceptance

- [ ] 6 主题 light/dark 各加 `--success` / `--danger`。
- [ ] ImportExport 无 `--color-*` / 硬编码 hex fallback。
- [ ] toast 成功/错误在 light/dark + 各主题可读。
- [ ] types.ts ThemeVariables 接口加 success/danger（若类型约束）。
- [ ] cargo/yarn build OK；check-i18n 零缺失（无新 key）。

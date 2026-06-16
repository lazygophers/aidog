# 弹窗本体背景 100% 不透明

## 背景
liquidGlass 主题 `--bg-elevated` = `rgba(255,255,255,0.82)` (light) / `rgba(30,30,34,0.8)` (dark) 半透明 + glass-elevated `backdrop-filter: blur` → 浮层本体透出背后内容。其他 5 主题 (nord/dracula/catppuccin/solarized) bg-elevated 纯色不透明。

用户要求：所有浮层（modal/popover/toast/dropdown）**本体背景 100% 不透明**。遮罩层 (overlay rgba(0,0,0,0.4~0.5)) 保持半透明。

## 方案
新增 CSS 变量 `--bg-floating`（浮层专用不透明背景）：
- liquidGlass light: `#ffffff`（纯白，去 0.82 alpha）
- liquidGlass dark: `#1e1e22`（纯暗，去 0.8 alpha）
- 其他 5 主题: 复用各自 `--bg-elevated` 纯色值（已不透明）

浮层容器 background 改 `var(--bg-floating)`。backdrop-filter 保留（不透明 bg 下无视觉影响，代码无害）。

## 改动范围

### 1. 主题变量（src/themes/*.ts）
6 主题（liquidGlass/catppuccin/dracula/nord/solarized）light + dark 各加 `--bg-floating`。

### 2. globals.css
- `.glass-elevated` background: var(--bg-elevated) → var(--bg-floating)
- `.toast` background → var(--bg-floating)

### 3. popover.css
- `.popover-root` background: linear-gradient(var(--bg-elevated)×2), var(--bg-base) → var(--bg-floating)

### 4. inline modalBody / 浮层容器（src/**/*.tsx）
- Mcp.tsx:322/442/625/890 background: "var(--bg-elevated)" → "var(--bg-floating)"
- ImportExport.tsx:160/219 toast → var(--bg-floating)
- editors.tsx / Platforms.tsx / ModelTestPanel.tsx / Skills.tsx modal 本体（exec 时 grep 补全）
- TrayConfigTab.tsx:322 rgba(30,30,30,0.95) 预览（exec 判定是否改）

### 5. 不动
- 遮罩层 rgba(0,0,0,0.4~0.5)（NotificationSettings/editors×2/UnsavedChanges/Platforms/ModelTest/Mcp/Skills×4）
- glass-surface（section card，非浮层）

## 验证
- yarn build（tsc）
- grep 确认无浮层容器残留 var(--bg-elevated) 直接背景（浮层该用 --bg-floating）
- 6 主题 light/dark --bg-floating 不透明（无 alpha）

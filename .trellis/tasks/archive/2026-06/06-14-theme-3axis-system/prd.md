# 主题系统 3 轴化：style × color × dark/light

## Goal

把当前「扁平 2 轴」主题（`themeName`(5) × `mode`(2)，每个 themeName 文件混了结构+色彩变量）重构为 **3 轴**：`style`(结构/材质) × `color`(调色板) × `mode`(light/dark)。主题 = 任意 style + 任意 color + mode 自由组合。新增 3 style + 8 palette。

## What I already know（现状 + 精确锚点）

- `src/themes/`：5 个主题文件各 `{name,label,light:{vars},dark:{vars}}`，**每个文件同时定义结构变量 + 色彩变量**（耦合）。
  - 结构类变量：`--radius-sm/md/lg/xl` `--glass-blur` `--glass-saturate` `--glass-border` `--shadow-sm/md/lg` `--transition`
  - 色彩类变量：`--bg-base/elevated/floating/glass/glass-hover/surface` `--text-primary/secondary/tertiary` `--accent/accent-hover/accent-subtle` `--success` `--danger` `--border` `--border-focus`
  - 不一致：nord 有 `--bg-floating`，liquidGlass 无 → 需统一变量集。
- `src/themes/types.ts`：`ThemeMode`/`ThemeName`/`ThemeDefinition` + `applyThemeVars`/`clearThemeVars`。
- `src/themes/index.ts`：`themeMap` + `getAvailableThemes()` + `applyTheme(name, mode)`（clear 旧 → apply 新 → 写 `data-theme`/`data-mode`）。
- `src/context/AppContext.tsx`：`settings={locale, themeName, themeMode}`（默认 `liquidGlass`/`light`），`setThemeName`/`setThemeMode`/`toggleMode`，effect `applyTheme(themeName, themeMode)`。
- `src/components/Sidebar.tsx:377-411`：主题下拉（列 `availableThemes`，按 `th.name===themeName` 高亮）+ dark/light 切换钮。
- `src/popover.tsx`：popover 独立窗口也 applyTheme（需同步 3 轴）。
- 当前 5 主题真实身份：**liquidGlass = 1 个 style + Apple 蓝调色板**；**nord/dracula/catppuccin/solarized = 4 个调色板（共享类 flat 结构）**。

## Decision (ADR-lite)

**Context**: style 与 color 当前耦合在 themeName，无法组合（不能「liquidGlass 结构 + nord 配色」）。
**Decision**: 解耦为两个独立注册表 `styles/`（结构）+ `palettes/`（色彩），`applyTheme(styleId, colorId, mode)` 合并 `style[mode] ∪ palette[mode]` 写 CSS 变量。shadow/glass 边的「色」由 palette 经 `--shadow-rgb`/`--glass-edge` 提供，「形」由 style 经 `--shadow-*`/`--glass-border` 用 `rgba(var(--shadow-rgb),α)` 组合 —— 干净解耦。AppContext 存 `{themeStyle, themeColor, themeMode}`，旧 `themeName` 一次性迁移。
**Consequences**: 4×12×2 = 96 组合，定义只需 4 style + 12 palette 文件。Sidebar 改双选择器（style + color）+ 保留 mode 钮。

## 变量分类契约（实现必须严格遵守）

### Palette（color 轴）提供的变量集（**全 12 palette 必须各定义 light+dark 全集**）
```
--bg-base --bg-elevated --bg-floating --bg-glass --bg-glass-hover --bg-surface
--text-primary --text-secondary --text-tertiary
--accent --accent-hover --accent-subtle
--success --danger
--border --border-focus
--shadow-rgb      /* "r, g, b" 三通道（供 style 的 shadow 用 rgba(var(--shadow-rgb),α)）。深色阴影用 "0, 0, 0" 或调色板暗色 tint */
--glass-edge      /* rgba(...) 玻璃/高光边颜色，供 style 的 --glass-border 用 */
```
派生规则（减少手工、保一致）：`accent-hover` = accent 明度 ±10%；`accent-subtle` = accent @ alpha .12–.15；`border` = text-primary @ alpha .10–.15；`border-focus` = accent @ alpha .4–.5。`bg-floating` 缺省 = bg-elevated。

### Style（结构轴）提供的变量集（light+dark，差异仅在 shadow/glass alpha）
```
--radius-sm --radius-md --radius-lg --radius-xl
--glass-blur --glass-saturate
--glass-border         /* 例: 1px solid var(--glass-edge) 或 1px solid var(--border) */
--shadow-sm --shadow-md --shadow-lg   /* 用 rgba(var(--shadow-rgb), α) 组合 */
--transition
```

### 4 个 Style 结构规格
| style | radius sm/md/lg/xl | blur | saturate | glass-border | shadow 风格 | transition |
|---|---|---|---|---|---|---|
| **liquidGlass**(保留现值) | 10/14/20/28 | 24px | 1.8(light)/1.6(dark) | `1px solid var(--glass-edge)` | 多层柔阴影 `0 4px 12px rgba(var(--shadow-rgb),.06/.4)`… | 250ms cubic-bezier(.4,0,.2,1) |
| **flat 极简** | 6/8/10/12 | 0 | 1 | `1px solid var(--border)` | 极轻单层 `0 1px 2px rgba(var(--shadow-rgb),.06/.25)` | 150ms ease |
| **soft 柔拟态** | 12/16/22/28 | 0 | 1 | `1px solid var(--glass-edge)`(极淡) | 双向柔阴影(neumorphic 凸感) `6px 6px 14px rgba(var(--shadow-rgb),.10/.45), -6px -6px 14px rgba(255,255,255,.04)` | 200ms ease |
| **sharp 硬郭** | 0/0/2/2 | 0 | 1 | `1.5px solid var(--text-primary)` | 硬投影 `3px 3px 0 rgba(var(--shadow-rgb),.85)` 或近无 | 100ms linear |

> 注意 blur=0 时 backdrop-filter 失效，玻璃面靠 palette 的 bg-glass alpha 呈现；flat/sharp 视觉更实，符合预期（v1 接受，不额外改 bg-glass）。

### 12 个 Palette 色值规格

**已有 4（保留，仅去掉结构变量、补 shadow-rgb/glass-edge）**：nord / dracula / catppuccin / solarized —— 沿用现有 bg/text/accent 值。

**established 4（用官方 canonical hex，不确定的 WebSearch「<name> palette hex official」核对）**：
- **appleBlue**（从 liquidGlass 抽色）：light bg-base `#f0f0f3` surface `rgba(255,255,255,.88)` text `rgba(0,0,0,.88/.5/.3)` accent `#007AFF` hover `#0056CC` success `#34C759` danger `#FF3B30` glass-edge `rgba(255,255,255,.35)` shadow-rgb `0,0,0`；dark bg `#0a0a0c` surface `rgba(28,28,32,.85)` text `rgba(255,255,255,.93/.55/.3)` accent `#4A9EFF` hover `#6BB3FF` success `#30D158` danger `#FF453A` glass-edge `rgba(255,255,255,.07)` shadow-rgb `0,0,0`。
- **rosePine**：dawn(light) base `#faf4ed` surface `#fffaf3` text `#575279/#797593` accent(rose) `#d7827e` (iris)hover `#907aa9` success(pine) `#286983` danger(love) `#b4637a`；main(dark) base `#191724` surface `#1f1d2e` text `#e0def4/#908caa` accent `#ebbcba` hover `#c4a7e7` success `#31748f` danger `#eb6f92`。
- **tokyoNight**：day(light) bg `#e1e2e7` surface `#d0d5e3` text `#3760bf/#6172b0`(用深蓝灰 `#343b58` 作 primary) accent `#2e7de9` success `#587539` danger `#f52a65`；night(dark) bg `#1a1b26` surface `#24283b` text `#c0caf5/#9aa5ce` accent `#7aa2f7` hover `#bb9af7` success `#9ece6a` danger `#f7768e`。
- **gruvbox**：light bg `#fbf1c7` surface `#f2e5bc` text `#3c3836/#665c54` accent `#d65d0e`(orange) hover `#af3a03` success `#79740e` danger `#9d0006`；dark bg `#282828` surface `#32302f` text `#ebdbb2/#a89984` accent `#fe8019` hover `#d65d0e` success `#b8bb26` danger `#fb4934`。

**bespoke 4（我考据授色 —— 严格用下列 anchor，agent 按派生规则补 hover/subtle/border/floating）**：
- **morandi 莫兰迪**（低饱和灰调）：
  - light: bg-base `#ECE8E3` surface `#F4F1EC` elevated `#F0ECE6` text `#4A4540/#6E665E/#9A9088` accent `#B08A7E`(尘玫) success `#8C9A82`(鼠尾草绿) danger `#B5746B`(陶土) glass-edge `rgba(255,255,255,.5)` shadow-rgb `74,69,64`
  - dark: bg-base `#2B2926` surface `#34322E` elevated `#302E2A` text `#E2DDD6/#B5AFA6/#857F77` accent `#C29C8F` success `#9DAA92` danger `#C28178` glass-edge `rgba(255,255,255,.05)` shadow-rgb `0,0,0`
- **monet 莫奈**（印象派睡莲，柔蓝绿紫）：
  - light: bg-base `#EAF0F1` surface `#F2F6F6` elevated `#EDF2F2` text `#3A4A52/#5E727B/#90A2AA` accent `#6E92B8`(睡莲蓝) success `#7FA284`(池绿) danger `#C98A86`(暖玫) glass-edge `rgba(255,255,255,.55)` shadow-rgb `58,74,82`
  - dark: bg-base `#1E2A30` surface `#273840` elevated `#22323A` text `#DCE6E8/#A6B8BD/#7A8C92` accent `#8AAECB` success `#93B597` danger `#D29C98` glass-edge `rgba(255,255,255,.05)` shadow-rgb `0,0,0`
- **wafu 和风**（日本传统色：藍/茜/生成り/墨/浅葱）：
  - light: bg-base `#FBF8F1`(生成り) surface `#FFFFFF` elevated `#F7F3EC` text `#2B2B2B`(墨)/`#5A5750`/`#8A857C` accent `#1F5C8B`(藍) success `#6E8B5A`(松葉) danger `#9E2236`(茜) glass-edge `rgba(255,255,255,.5)` shadow-rgb `43,43,43`
  - dark: bg-base `#1A2230`(紺) surface `#232B3A` elevated `#1F2735` text `#F0EAE0/`#B8B2A6`/`#857F75` accent `#4FA3AD`(浅葱) success `#88A06E` danger `#C24E5C`(今様) glass-edge `rgba(240,234,224,.06)` shadow-rgb `0,0,0`
- **guofeng 中国风**（中国传统色：月白/胭脂/朱砂/竹青/黛/藤黄）：
  - light: bg-base `#F2EFE6`(象牙) surface `#FAF7EF` elevated `#EDEAE0` text `#2E2C2B`(黛)/`#5C5853`/`#8C867D` accent `#9D2933`(胭脂) hover `#7E1F28` success `#6B8E5A`(竹青) danger `#C0392B`(朱砂) glass-edge `rgba(255,255,255,.5)` shadow-rgb `46,44,43`
  - dark: bg-base `#1C1A18`(玄) surface `#262320` elevated `#211E1B` text `#E6EBE8`(月白)/`#B0AAA0`/`#827C72` accent `#D8503C`(朱砂) hover `#E6A817`(藤黄) success `#7FA968`(石绿) danger `#C0392B` glass-edge `rgba(230,235,232,.06)` shadow-rgb `0,0,0`

## Requirements

### 后端无关，纯前端。分两阶段（foundation → palettes，后者依赖前者契约）。

**阶段 1 · foundation（架构+styles+UI+迁移，先做）**
- `src/themes/types.ts`：拆 `ThemeStyle`(联合 4) / `ThemeColor`(联合 12) / `ThemeMode`；`StyleDefinition{id,label,light,dark}` / `PaletteDefinition{id,label,light,dark}`。保留 apply/clear helper。
- `src/themes/styles/`：4 文件（liquidGlass/flat/soft/sharp），仅结构变量，按上表。
- `src/themes/palettes/`：先把现有 nord/dracula/catppuccin/solarized **去结构变量**移入 + 新增 appleBlue（共 5），其余 7 个阶段 2 补。
- `src/themes/index.ts`：`styleMap`/`paletteMap` + `getAvailableStyles()`/`getAvailableColors()` + `applyTheme(style, color, mode)`（clear 全部已知变量 → apply `palette[mode]` then `style[mode]` → 写 `data-theme-style`/`data-theme-color`/`data-mode`）。
- `src/context/AppContext.tsx`：`settings.{themeStyle,themeColor,themeMode}`，新 setter `setThemeStyle`/`setThemeColor`/`toggleMode`。**迁移**：读到旧 `themeName` → 映射（`liquidGlass`→{liquidGlass,appleBlue}；`nord/dracula/catppuccin/solarized`→{flat,同名}）写回新结构。
- `src/components/Sidebar.tsx`：主题区改 **style 选择器 + color 选择器**两个下拉（或分组），保留 mode 钮。
- `src/popover.tsx`：同步 3 轴 applyTheme + 读新 settings。
- i18n：`theme.style.*`(4) + `theme.color.*`(12) label key，8 locale（ar/de/en/es/fr/ja/ru/zh）全加。品牌/调色板专名（Nord/Dracula/Tokyo Night…）保留原文，中文 label 给「莫兰迪/莫奈/和风/中国风/苹果蓝」等。

**阶段 2 · palettes（补 7 新调色板，依赖阶段 1 契约）**
- `src/themes/palettes/` 加 rosePine/tokyoNight/gruvbox/morandi/monet/wafu/guofeng 7 文件，按上表色值 + 派生规则补全变量集。
- 注册进 `paletteMap` + `ThemeColor` 联合 + i18n label。

## Acceptance Criteria

- [ ] `applyTheme(style,color,mode)` 任意组合（如 sharp+guofeng+dark、liquidGlass+morandi+light）正确合并写变量，无残留旧变量。
- [ ] 4 style × 12 palette × 2 mode 全部可选、可切换、即时生效；popover 同步。
- [ ] 旧 `themeName` 持久化用户升级后自动迁移到 `{themeStyle,themeColor}`，不白屏不丢配置。
- [ ] 每个 palette 定义完整变量集（含 bg-floating/shadow-rgb/glass-edge），无 undefined 变量导致样式塌陷。
- [ ] bespoke 4 调色板色值严格按 PRD anchor；established 用官方 canonical。
- [ ] Sidebar 双选择器 + mode 钮交互正常；i18n 8 locale 无缺失（`yarn check:i18n` 过）。
- [ ] `yarn build` 通过，无 tsc warning，无 `any`。
- [ ] 抽查截图：liquidGlass+appleBlue 与重构前视觉一致（回归基线）。

## Out of Scope

- 后端 / Rust（纯前端 CSS 变量）。
- 新增超出本 PRD 的 style/palette。
- 主题编辑器 / 用户自定义调色板（未来）。
- 改 bg-glass alpha 以适配 flat/sharp 的实色感（v1 接受玻璃面在非 glass style 下仍半透）。

## Technical Notes

- code-reuse：palette/style 各走统一模块模板，禁逐文件复制结构变量（结构归 style）。读 `.trellis/spec/guides/code-reuse-rules.md`。
- frontend/conventions：组件/状态/类型/i18n（无 any）。读 `.trellis/spec/frontend/conventions.md`。
- clearThemeVars 需清「style+palette 全量已知键」并集，避免切换残留（如从 liquidGlass 切 flat 后 blur 残留）。
- 主题切换是热路径但低频，无性能约束。
- 验证手动：dev → Sidebar 切 style/color/mode 组合 → 肉眼 + Playwright 截图抽查 bespoke 4。

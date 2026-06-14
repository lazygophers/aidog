# PRD: 主题配色 palette 换具体命名色板

## 目标

移除 4 个抽象/风格化命名 palette (morandi/monet/wafu/guofeng), 换成 4 个业界知名具体命名色板 (One Dark / Material Theme / GitHub / Night Owl), 使色彩轴全部为可识别、可复现的具体配色方案, 与现有 nord/dracula/catppuccin/solarized/rosePine/tokyoNight/gruvbox 风格一致。

## 现状锚点

- `src/themes/types.ts:16-28` ThemeColor 联合 12 变体 (8 命名 + 4 抽象)。
- `src/themes/index.ts:11-22` import 4 抽象 palette 文件; `:61-74` paletteMap 注册 12。
- `src/themes/palettes/{morandi,monet,wafu,guofeng}.ts` 4 抽象 palette 定义。
- 8 locale 文件 `theme.color.{morandi,monet,wafu,guofeng}` label 行。
- 全仓 grep 确认: palette id 仅被 types.ts + index.ts 引用, 无 tsx/rs 硬编码 id。
- DB setting (scope=app, key=theme) 可能存旧 id → `applyTheme` resolvePalette 未注册 → fallback `appleBlue` (DEFAULT_COLOR), 安全降级。

## 替换映射

| 旧 (移除) | 新 (新增)       | 来源色板                          |
| --------- | --------------- | --------------------------------- |
| morandi   | `oneDark`       | Atom One Dark (dark) / One Light (light) |
| monet     | `material`      | Material Theme Palenight (dark) / Material Lighter (light) |
| wafu      | `github`        | GitHub Dark Default / GitHub Light |
| guofeng   | `nightOwl`      | Night Owl (dark) / Light Owl (light) |

> 映射保留位置不变 (paletteMap 顺序), 仅替换文件 + id + label。

## 色板规格 (实现锚点)

### oneDark (Atom One Dark / One Light)
- dark: bg #282C34, elevated #21252B, fg #ABB2BF, accent #61AFEF, success #98C379, danger #E06C75, yellow #E5C07B, purple #C678DD
- light: bg #FAFAFA, fg #383A42, accent #4078F2, success #50A14F, danger #E45649

### material (Material Theme Palenight / Lighter)
- dark: bg #292D3E, fg #EEFFFF, accent #82AAFF, success #C3E88D, danger #F07178, purple #C792EA
- light: bg #FAFAFA, fg #EEFFFF(lighter 用 #272727 友好), accent #82AAFF

### github (GitHub Dark Default / GitHub Light)
- dark: bg #0D1117, elevated #161B22, fg #C9D1D9, accent #58A6FF, success #3FB950, danger #F85149, border rgba(240,246,252,0.1)
- light: bg #FFFFFF, elevated #F6F8FA, fg #1F2328, accent #0969DA, success #1A7F37, danger #CF222E

### nightOwl (Night Owl / Light Owl)
- dark: bg #011627, elevated #112639, fg #D6DEEB, accent #82AAFF(或 #7E57C2 紫), success #22DA6E, danger #EF5350, cyan #7FDBCA
- light: bg #FAFBFC, fg #403F53, accent #4373EE, success #2D9C5F, danger #E64545

> 每个色板补齐完整 CSS 变量集 (参照 `appleBlue.ts`/`morandi.ts` 的 key 列表): `--bg-base/--bg-elevated/--bg-floating/--bg-glass/--bg-glass-hover/--bg-surface/--text-primary|secondary|tertiary/--accent|--accent-hover|--accent-subtle/--accent-1..5/--accent-gradient/--success/--danger/--border/--border-focus/--shadow-rgb/--glass-edge`。light/dark 各一组。

## 实施步骤

### Phase 1 · 新 palette 文件 (worktree)
1. 新建 4 文件: `src/themes/palettes/{oneDark,material,github,nightOwl}.ts`, 按规格填全 CSS 变量 (light+dark)。
2. 删 4 旧文件: `src/themes/palettes/{morandi,monet,wafu,guofeng}.ts`。

### Phase 2 · 类型 + 注册表
3. `src/themes/types.ts:16-28` ThemeColor 联合: `morandi|monet|wafu|guofeng` → `oneDark|material|github|nightOwl`。
4. `src/themes/index.ts`:
   - import 4 旧 → 4 新。
   - paletteMap 注册 4 旧 → 4 新 (位置不变)。

### Phase 3 · i18n label
5. 8 locale 文件: `theme.color.{morandi,monet,wafu,guofeng}` key 改 `{oneDark,material,github,nightOwl}`, value 按语言本地化:
   - oneDark: zh "One Dark (Atom)" / en "One Dark (Atom)" / ...
   - material: zh "Material Theme" / en "Material Theme" / ...
   - github: zh "GitHub" / en "GitHub" / ...
   - nightOwl: zh "Night Owl" / en "Night Owl" / ...

### Phase 4 · 迁移 (旧 DB 值兼容)
6. `src/context/AppContext.tsx` loadSettingsFromDB themeRow.style/color 读取后, 若 color ∈ {morandi,monet,wafu,guofeng} → 映射 {oneDark,material,github,nightOwl} (按上表), 保证老用户偏好不丢失 (而非无解释 fallback appleBlue)。

## 验收标准

- [ ] `yarn build` 通过 (tsc + vite), ThemeColor 联合无残留旧 id。
- [ ] grep 全仓 `morandi|monet|wafu|guofeng` 仅剩 (若有) commit history / archived task (src 内清零)。
- [ ] 4 新 palette 文件存在, 每个含完整 CSS 变量集 (light+dark ≥ 25 key)。
- [ ] 8 locale 文件 `theme.color.{oneDark,material,github,nightOwl}` 4 key 各加, 旧 4 key 各删。
- [ ] AppContext 迁移映射覆盖旧 id → 新 id。
- [ ] 主题选择 UI 渲染 12 palette, 4 新色板视觉与官方色板一致 (肉眼核 dark/light 各档)。

## 风险

- **CSS 变量遗漏**: 新 palette 漏填某 key → 该变量沿用上次的值或空 → 视觉错位。缓解: 以 appleBlue.ts 的 key 列表为模板逐 key 填, 不省略。
- **色值不准**: 凭记忆配色与官方有偏差。缓解: 按上游仓库 (Atom One Dark / Material Theme / Primer GitHub / Night Owl 官方) 取精确 hex, 不猜。
- **locale value 风格不一**: 某些语言 "Night Owl" 直译怪。缓解: 品牌名保留英文 (与 rosePine/tokyoNight 先例一致), 不强译。
- **DB 旧值用户**: 无迁移则偏好突变为 appleBlue。缓解: Phase 4 显式映射。

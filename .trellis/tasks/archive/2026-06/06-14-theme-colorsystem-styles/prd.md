# 主题增强：调色板色系多色化 + 新增 5 style

## Goal

承接 06-14-theme-3axis-system。两件事：
1. **色系多色化**：用户反馈「中国风/莫奈/和风/莫兰迪是色系（一组协调多色），不是单一 accent」。给全 12 palette 各加多 accent token（`--accent-1..5` + `--accent-gradient`），从家族真实色考据，让主题读出「多色家族」。
2. **新增 5 style**：style 轴 4→9，加 Aurora 极光 / Paper 纸感 / Terminal 终端 / Bento 便当 / Sketchy 手绘风。Aurora 消费 `--accent-gradient` 作流光背景，最能展现色系多色。

## What I already know（现状锚点）

- 3 轴架构已落地：`src/themes/styles/*`(4) + `src/themes/palettes/*`(12) + `index.ts`(styleMap/paletteMap/`applyTheme(style,color,mode)`/`ALL_KNOWN_KEYS` 清残留)+ `types.ts`(ThemeStyle 4联合/ThemeColor 12联合/StyleDefinition/PaletteDefinition) + AppContext/popover(3轴+迁移) + Sidebar(双选择器)。
- palette 当前变量集（18 键）：bg-base/elevated/floating/glass/glass-hover/surface, text-primary/secondary/tertiary, accent/accent-hover/accent-subtle, success, danger, border, border-focus, **shadow-rgb, glass-edge**。
- style 当前变量集：radius-sm/md/lg/xl, glass-blur, glass-saturate, glass-border, shadow-sm/md/lg, transition。shadow 用 `rgba(var(--shadow-rgb),α)`、glass-border 用 `var(--glass-edge)`/`var(--border)`（解耦契约）。
- app 背景接入点：`src/styles/globals.css:17/44/348` `background: var(--bg-base)` —— Aurora 的 `--app-bg-overlay` 在此叠加。
- app 当前**只有语义色**（`--color-success/warning/danger/neutral` via colorScale.ts），**无分类/图表多色面** → 本任务多 accent token 主要供 Aurora 渐变/高光/装饰 + 未来分类用，**不强制 rewire 现有 stat chip**（用户选 a「每 palette 加多 token」非 c「接入现有 UI」）。

## Decision (ADR-lite)

**Context**: 色系需多色表达但 app 无多色出口；style 需扩充。
**Decision**:
- palette 契约扩展：加 `--accent-1 --accent-2 --accent-3 --accent-4 --accent-5`（家族协调色，accent-1 = 主 accent 同值保持向后兼容）+ `--accent-gradient`（linear-gradient 用其中 3 色）。全 12 palette 各按家族考据授色。
- style 契约扩展：加 `--app-bg-overlay`（默认 `none`；Aurora 用 accent token 的低 alpha 径向/线性渐变层）。globals.css 3 处 bg 改 `background: var(--app-bg-overlay, none), var(--bg-base);`（overlay 在上、bg-base 兜底；非 Aurora style overlay=none 等价原样）。
- ThemeStyle 4→9，加 aurora/paper/terminal/bento/sketchy 5 个 style 文件。
**Consequences**: 96→9×12×2=216 组合。多 accent token 立即被 Aurora 消费可见；其余 style 不变行为（overlay=none）。不 rewire 现有语义色 UI。

## 契约：新增变量

### palette 新增（全 12 各定义 light+dark）
```
--accent-1 --accent-2 --accent-3 --accent-4 --accent-5   /* 家族 5 协调色，accent-1=现 --accent 同值 */
--accent-gradient                                          /* linear-gradient(135deg, <c1> 0%, <c2> 50%, <c3> 100%) */
```
### style 新增（全 9 各定义；非 Aurora = none）
```
--app-bg-overlay   /* 默认 none；Aurora=多层 radial/linear gradient 用 var(--accent-1..3) @ 低 alpha */
```

## 色族规格（12 palette 的 accent-1..5 + gradient）

> accent-1 = 现有 `--accent` 值不变。bespoke 4 严格按下列考据 anchor；established 8 用家族官方多色集（不确定 WebSearch「<name> palette accents hex」核对）。light/dark 可同一组家族色或按模式微调明度（dark 略提亮）。gradient 取家族中 3 个有张力的色。

**bespoke 4（考据授色，严格用）**
- **guofeng 中国风**（中国传统色）：胭脂`#9D2933` 朱砂`#C0392B` 藤黄`#E6A817` 竹青`#6B8E5A` 群青`#2E59A7`。gradient 胭脂→藤黄→竹青。dark 各提亮 ~8%（朱砂`#D8503C` 等）。
- **monet 莫奈**（印象派睡莲）：睡莲蓝`#6E92B8` 池绿`#7FA284` 薰衣草`#A88FB5` 暖桃`#E0A87E` 柳绿`#9FB46A`。gradient 蓝→薰衣草→绿。
- **wafu 和风**（日本传统色）：藍`#1F5C8B` 茜`#9E2236` 浅葱`#4FA3AD` 山吹`#E8A33D` 利休鼠`#8B9A7B`。gradient 藍→浅葱→山吹。
- **morandi 莫兰迪**（低饱和）：尘玫`#B08A7E` 鼠尾草`#8C9A82` 雾蓝`#8E9DAB` 陶土`#B5746B` 灰褐`#A9947E`。gradient 尘玫→雾蓝→鼠尾草。

**established 8（官方家族多色集，取 5）**
- nord：`#88C0D0 #81A1C1 #A3BE8C #EBCB8B #B48EAD`（frost+aurora）
- dracula：`#FF79C6 #BD93F9 #8BE9FD #50FA7B #FFB86C`
- catppuccin：`#cba6f7 #89b4fa #a6e3a1 #fab387 #f38ba8`（mocha；latte light 同名稍深）
- solarized：`#268bd2 #2aa198 #859900 #b58900 #d33682`
- gruvbox：`#fb4934 #fabd2f #b8bb26 #8ec07c #d3869b`
- tokyoNight：`#7aa2f7 #bb9af7 #7dcfff #9ece6a #e0af68`
- rosePine：`#eb6f92 #f6c177 #9ccfd8 #c4a7e7 #31748f`
- appleBlue：`#007AFF #34C759 #FF9500 #FF3B30 #AF52DE`（Apple 系统色）

## Style 结构规格（5 新）

| style | radius sm/md/lg/xl | blur | glass-border | shadow | app-bg-overlay | transition |
|---|---|---|---|---|---|---|
| **aurora 极光** | 12/16/22/30 | 30px | `1px solid var(--glass-edge)` | 柔光 `0 8px 32px rgba(var(--shadow-rgb),.10/.45)` | **多层渐变**: `radial-gradient(60% 50% at 20% 0%, color-mix(in srgb, var(--accent-1) 22%, transparent), transparent), radial-gradient(50% 40% at 90% 10%, color-mix(in srgb, var(--accent-3) 18%, transparent), transparent), linear-gradient(160deg, color-mix(in srgb, var(--accent-2) 10%, transparent), transparent)` | 300ms cubic-bezier(.4,0,.2,1) |
| **paper 纸感** | 4/6/8/10 | 0 | `1px solid var(--border)` | `0 1px 3px rgba(var(--shadow-rgb),.10), 0 8px 24px rgba(var(--shadow-rgb),.04)` | `none`（或极淡 `repeating-linear-gradient` 纸纹 @ <.02 alpha） | 200ms ease |
| **terminal 终端** | 0/2/2/4 | 0 | `1px solid color-mix(in srgb, var(--accent-1) 50%, transparent)` | `0 0 0 1px rgba(var(--shadow-rgb),.4)` | 扫描线 `repeating-linear-gradient(0deg, rgba(var(--shadow-rgb),.03) 0 1px, transparent 1px 3px)` | 80ms linear |
| **bento 便当** | 16/20/26/32 | 0 | `1.5px solid var(--border)` | `0 2px 8px rgba(var(--shadow-rgb),.10)` | `none` | 200ms ease |
| **sketchy 手绘** | `12px 10px 14px 8px`(md 类不规则) / sm `8px 6px 9px 7px` / lg `18px 14px 20px 16px` / xl `26px 20px 28px 22px` | 0 | `2px solid var(--text-primary)`（墨线描边） | `2px 3px 0 rgba(var(--shadow-rgb),.7)`（马克笔偏移投影） | 120ms ease |

> color-mix(in srgb, …) 在 Tauri webview(WebKit/WebView2) 支持（项目已用，见 colorScale 语义色注释）。
> 局限：terminal/sketchy 理想要等宽/手写字体，但字体非现有主题变量（不在本任务范围）—— 仅做结构/边/投影表达，PRD 注明。

## Requirements

纯前端。两阶段（palettes 多色 → styles，皆改 types/index 故串行）。

**阶段 1 · palettes 多色（先）**
- `types.ts`：`PaletteDefinition` 文档/类型反映新增 accent-1..5 + accent-gradient（若用 Record<string,string> 则无需改类型，仅约定键）。
- 全 12 `palettes/*.ts`：light+dark 各加 `--accent-1..5` + `--accent-gradient`，按色族规格。accent-1 = 现 --accent。
- `index.ts` `ALL_KNOWN_KEYS`：自动纳入新键（基于 getAvailableColors 全键并集）——确认清残留覆盖新 token。
- 无需新 i18n（token 无 label）。

**阶段 2 · styles 扩充（后，依赖阶段 1 的 accent-gradient/accent-1..3）**
- `types.ts`：`ThemeStyle` 4→9（+aurora/paper/terminal/bento/sketchy）。
- 5 个 `styles/*.ts`：按结构规格，各定义全 style 变量集 + `--app-bg-overlay`。**现有 4 style 也补 `--app-bg-overlay: none`**（保证 clear/apply 键集一致，切走 Aurora 后 overlay 清掉）。
- `index.ts`：注册 5 style 进 styleMap；ALL_KNOWN_KEYS 含 app-bg-overlay。
- `src/styles/globals.css`：3 处 `background: var(--bg-base)` → `background: var(--app-bg-overlay, none), var(--bg-base)`（注意 background 简写会覆盖，确认不破坏既有 background-color 语义；必要时拆 `background-image`/`background-color` 两行）。
- i18n 8 locale：`theme.style.{aurora,paper,terminal,bento,sketchy}`。中文：极光/纸感/终端/便当/手绘。
- Sidebar style 选择器自动列 9（getAvailableStyles 驱动）。

## Acceptance Criteria

- [ ] 12 palette 各定义 `--accent-1..5` + `--accent-gradient`（light+dark），bespoke 4 严格按 anchor、established 用官方多色。
- [ ] 9 style 各定义全变量集含 `--app-bg-overlay`；现有 4 style overlay=none。
- [ ] Aurora + 任意 palette：app 背景出现该色系多色流光渐变（消费 accent-gradient/accent-1..3）；切到非 Aurora style overlay 清除、背景恢复 bg-base。
- [ ] sketchy 卡片不规则圆角 + 墨线描边 + 偏移投影；terminal 扫描线 + accent 边；paper 柔纸投影；bento 大圆角粗分隔。
- [ ] 9 style × 12 palette × 2 mode 任意组合可切换、无残留、无 undefined 变量塌陷。
- [ ] globals.css 改动不破坏既有背景（抽查 liquidGlass+任意 palette 背景仍 bg-base）。
- [ ] `yarn build` + `yarn check:i18n` 通过（5 新 style key 全 8 locale），无 tsc warning，无 any。

## Out of Scope

- rewire 现有语义色 UI（stat chip/进度条）用多 accent（用户选 a，未来再说）。
- 字体维度（terminal 等宽 / sketchy 手写字体非主题变量）。
- 后端 / Rust。
- 再加更多 style/palette（本批锁定 5 style；"等等" 留后续任务）。

## Technical Notes

- code-reuse：palette/style 走既有模块模板，多 token 按家族统一加，禁逐文件硬编码无规律。
- 解耦契约延续：style 的 overlay/border/shadow 用 `var(--accent-*)`/`rgba(var(--shadow-rgb),α)` 引用 palette 色，禁在 style 写死家族色。
- ALL_KNOWN_KEYS 必须含所有新 token + app-bg-overlay，否则切换残留（Aurora→flat 后 overlay 不清 = 背景渐变残留）。
- globals.css background 简写陷阱：`background: <image>, <color>` 会重置 background-color；确认 3 处原值仅 `var(--bg-base)` 纯色，改写后等价。
- 验证手动：dev → Aurora+中国风/莫奈/和风 看流光；sketchy+任意看手绘；切回 liquidGlass 确认背景净。

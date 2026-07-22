# shadcn 基础设施+主题 token 体系 — 详细设计

架构 / 数据流 / 关键取舍 / 技术选型 (不含调度图, 调度归 task.json):

## 现状

- 无 tailwind / 无 shadcn / 无 Radix,仅 `clsx` (package.json)
- 主题系统: `src/themes/` 两轴正交 — Style (9, 结构: radius/blur/shadow/glass/transition) × Palette (12, 色彩 + shadow-rgb/glass-edge),108 组合,用户可切换 (产品功能)
- 运行时切换: `applyTheme(style,color,mode)` 组合 StyleDefinition.vars + PaletteDefinition.vars → `document.documentElement.style.setProperty` 注入 CSS 变量 (`types.ts::applyThemeVars`),切换前 `clearThemeKeys` 清并集避免残留
- 变量空间: `--bg-base/--bg-elevated/--bg-glass/--radius-sm/.../--glass-blur/--shadow-*/--transition` (非 shadcn 命名)
- globals.css 616 行 + popover.css 293 行,大量自定义类直消费这些变量

## 目标架构

Tailwind v4 + shadcn/ui (nova preset, base/radix) + **保留运行时主题切换**:

```
src/
  styles/globals.css     ← Tailwind v4 入口 (@import "tailwindcss") + @theme inline 块 (shadcn 标准 token)
  themes/
    types.ts             ← StyleDefinition/PaletteDefinition.vars 的 key 改投 shadcn 语义 token
    styles/*.ts (9)      ← 结构 token: --radius-sm/md/lg/xl + --glass-blur/saturate/border (Liquid Glass 扩展,非 shadcn 标准)
    palettes/*.ts (4)    ← 色彩 token: --background/--foreground/--primary/--muted/--border/--ring/... light+dark
    index.ts             ← applyTheme 不变, 组合后 setProperty (变量名已是 shadcn 语义)
  lib/utils.ts           ← cn() = clsx + tailwind-merge (shadcn 标准)
components.json          ← shadcn 配置 (aliases/base/style/iconLibrary)
```

## 数据流

```
用户在 Settings 选 (style, color, mode)
  → applyTheme(style, color, mode)
  → resolveStyle(style).vars + resolvePalette(color).vars (按 mode 取 light/dark)
  → clearThemeKeys(已知 token 并集)  // 清残留
  → applyThemeVars(merged)           // setProperty 到 :root
  → Tailwind v4 @theme inline 读 CSS 变量 → bg-background/text-primary 等 utility 实时更新
```

机制不变,仅 vars 的 **key 改名** (投到 shadcn 语义 token),值重映射。

## token 映射表

### Palette (色彩 → shadcn 语义色, 4 套 × light/dark)

| shadcn token | 语义 | 现来源 |
|---|---|---|
| `--background` / `--foreground` | 页面底/正文 | `--bg-base` + 正文色 |
| `--card` / `--card-foreground` | 卡片面 | `--bg-elevated` |
| `--popover` / `--popover-foreground` | 弹层 | `--bg-floating` |
| `--primary` / `--primary-foreground` | 主色 | palette 主题色 (gruvbox/nord/dracula/catppuccin 各自) |
| `--secondary` / `--muted` / `--accent` (+ foreground) | 次级/弱化/强调 | palette 派生 |
| `--destructive` / `--destructive-foreground` | 危险 | 红色固定 + palette 调和 |
| `--border` / `--input` / `--ring` | 边框/输入框/聚焦环 | palette 边线 + `--glass-edge` |
| `--shadow-color` (扩展) | 阴影色 rgb | `--shadow-rgb` |

4 调色板: **gruvbox + nord + dracula + cattpuccin** (用户拍板)。删 8: appleBlue/solarized/rosePine/tokyoNight/oneDark/material/github/nightOwl。

### Style (结构 → radius + glass 扩展, 9 套 × light/dark)

| token | 语义 |
|---|---|
| `--radius-sm/md/lg/xl` (shadcn 标准) | 圆角阶梯,各 Style 不同 (liquidGlass 大 / sharp 0 / soft 中) |
| `--glass-blur` / `--glass-saturate` / `--glass-border` (扩展) | Liquid Glass 毛玻璃参数,非 shadcn 标准 token,皮肤层消费 |

9 Style 全保留,各自调 radius 阶梯 + glass 参数。

## 关键取舍

1. **保留运行时切换 (vs 静态多 CSS 文件)** — 主题切换是产品功能,setProperty 注入 + Tailwind v4 读 CSS 变量天然兼容,零机制改动,仅变量改名。静态方案要 108 个 CSS 文件,不可行。
2. **glass-* 扩展 token 留在 :root** — shadcn 标准不含玻璃参数,Liquid Glass 皮肤层 (globals.css 自定义类 + 业务 wrapper) 消费。shadcn 组件本身用标准 token,玻璃效果通过 wrapper/className 叠加。
3. **调色板砍到 4** — 用户拍板。已选被删调色板的用户设置回退默认 (gruvbox),迁移代码: settings 读取 theme color ∈ 删除集 → 回退。
4. **cn() 落 lib/utils.ts** — shadcn 标准,与 components.json aliases 对齐 (`@/lib/utils`)。
5. **init 命令**: `npx shadcn@latest init` (Vite 模板自动识别, nova preset)。先装 `tailwindcss@^4 @tailwindcss/vite` (Tailwind v4 Vite 插件), vite.config.ts 加 plugin。

## tracer-bullet (端到端穿通)

infra 完成后验证整条路:
- `npx shadcn@latest add button` (唯一本 task add 的组件)
- Home.tsx 或 About.tsx 一处原生 `<button>` 换 `<Button>`
- 启动 dev, 切 style/color/mode,Button 视觉跟随变
- yarn build + yarn test 过

证明: shadcn 组件能 import、token 体系通、主题切换作用到 shadcn 组件。后续 primitives/pages 在此基础上展开。

## 可能性分支 (研究期留痕, 不进当前方案)

- **若未来加调色板**: 补一份 PaletteDefinition (shadcn 语义色 light/dark),加进 `palettes/` + `ThemeColor` union,零机制改。
- **若 Tailwind v5 发布**: `@theme inline` 语法可能变,届时升 @tailwindcss/vite。当前锁 v4。
- **若需 CSS @property 类型化** (动画过渡更顺): 给 `--radius-*` 等加 `@property` 声明类型,当前 YAGNI。
- **若玻璃效果要进 shadcn 组件内部**: 写 shadcn registry 自定义 variant (如 `glass`),当前靠 wrapper 叠加够用。

# Logo 背景透明 + Sidebar 换 logo + CSS 主题适配补齐

## 目标
1. 抠 `src-tauri/icons/source/logo.svg` 内嵌 PNG 的米白背景 → 透明，重嵌 SVG。
2. Sidebar 左上角 `<IconPaw>` 换成透明 logo。
3. 全仓 CSS 硬编码颜色补齐主题变量（globals.css + themes/*.css + 所有 .tsx 内联 style）。

## 现状（事实）
- `logo.svg` = SVG 外壳包 base64 内嵌 PNG（1143×1169，全 alpha=255）。背景米白 `(245-253, 240-249, 230-237)` 占 ~36%，主体浅褐 `(220,203,185)` 爪印 + 底部横条。文件 1.2M。
- `IconPaw`（`src/components/icons.tsx:115`）纯 path `fill=currentColor`，Sidebar.tsx:231 用。
- globals.css 硬编码：
  - `.btn-danger` L213-219 `#ff453a` + `rgba(255,69,58,...)` → 应 `var(--color-danger)` / `var(--color-danger-bg)`（变量 L29-30 已定义未用）
  - L154 `background:#34c759` 绿点 → `var(--color-success)`
  - L304 `background:#fff`
  - checkbox 无 `accent-color`
  - L95-122/138 多处 `rgba(255,255,255,*)` liquid glass 高光（评估是否变量化）

## 交付
1. **抠图脚本 + 透明产物**：解码内嵌 PNG → 米白背景 alpha=0（阈值 `r>233 & g>228 & b>218 & (r-g)<25 & (r-b)<30`，已 ASCII 验证形状正确）→ 抗锯齿羽化边缘 → 重嵌 SVG。输出 `public/logo.svg`（Sidebar 用）+ 覆盖 `src-tauri/icons/source/logo.svg`（源）。
2. **Sidebar 接入**：`Sidebar.tsx:228-233` IconPaw span → `<img src="/logo.svg">`（logo.svg 多色固定品牌色，不跟 currentColor）。
3. **CSS 补齐**：globals.css btn-danger/绿点/#fff/checkbox accent-color 改变量；扫所有 .tsx 内联 `#hex`/`rgba(` 硬编码改主题变量；审 themes/*.css 变量定义一致性。

## 验证
- `yarn build` 过（tsc + vite）
- 视觉：Sidebar logo 透明无白底；btn-danger 随主题 danger 色变；checkbox accent-color 跟主题 accent
- grep 残留硬编码：globals.css + .tsx 内联非白/非透明 rgba 清零（liquid glass 高光 rgba(255,255,255,*) 评估保留或变量化，结果记录）

## 资源
logo.svg / icons.tsx / Sidebar.tsx / globals.css / themes/*.css / 全部 src/**/*.tsx

## 依赖
无外部。PIL（已装）处理 PNG。

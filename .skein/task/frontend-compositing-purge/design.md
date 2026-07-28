# 前端常驻合成层与动画清除 — 详细设计

## 现状（实测 + 静态清点）

**实测**（[01]/[03]）：空闲前台 CPU **50.2%**（隐藏窗口 0.2%）；WebContent #1 = graphics 230MB + WebKit malloc **82MB**（预算 32MB，超 2.6×）。GPU 54% 采样落在 `CA::CG::DrawConicGradient::draw_color`；WebContent 每帧 `Document::resolveStyle` → `TreeResolver::resolvePseudoElement`。

**根因两层，必须分开治**：
1. **animation 是否 tick** → CPU（逐帧 style 重算 + 软件光栅化）
2. **合成层是否存在** → GPU 内存（`backdrop-filter` 强制独立层；`mask-composite: exclude` 强制离屏 buffer，**`opacity: 0` 仍占层**）

[03] 实验已证：把 mask 挪进 hover，graphics 字节 231→230MB **几乎无变化** → 合成面主要由窗口面积决定（归 `window-default-size`），本 task 主攻 **CPU + WebKit malloc**。

### 常驻动画清单

| 位置 | 内容 |
|---|---|
| `globals.css:125` | 全仓唯一 `will-change` |
| `globals.css:126` | `bgShimmer 32s`（`body::before`） |
| `globals.css:151-154` | `pulseGlow` —— 动 `box-shadow`，非 compositor-only |
| `globals.css:182` | skeleton shimmer |
| `globals.css:843-865` | `.glass::after` conic + `mask-composite: exclude`，**116+ 元素各一离屏 buffer** |
| `globals.css:870` | `flowBorder`（仅 hover） |
| `globals.css:916` | `progressStripes` |
| `globals.css:135-137` | reduced-motion 块 —— **只关 `body::before`** |
| `globals.css:923-926` | 第二个 reduced-motion 块 |
| `pages/AppSettings/ProxyStatusSection.tsx:27` | **内联 `animation: "pulseGlow 3s infinite"` —— 漏 reduced-motion 网**（内联 style 的 animation 一律不受 CSS 媒体查询覆盖） |
| `components/PopoverCards.tsx:92` | animation |
| `components/Sidebar.tsx:276` | `filter: drop-shadow` |

### backdrop-filter 分布

| 位置 | blur | 引用数 |
|---|---|---|
| `globals.css:228-231` `.glass-elevated` | 30px | 51 处 tsx |
| `globals.css:289` `.btn` | 12px | 12 处 |
| `globals.css:365` `.input` | 12px | 47 处 |
| `SectionAnchorNav.tsx:35-36`、`SettingsHeader.tsx:78-79`、`CodexSettings.tsx:163-164` | 20px | 各 1 |

### bundle

`dist/assets/main-trwYgpvB.js` **1.6M**（实测）+ `window-DyisxYjm.js` 483.8K + CSS 55K；locale 已分包（ar-SA 136.2K … ru-RU 154.2K）。`vite.config.ts` 无 `manualChunks`。**bundle 拆分归 `cold-start-unblock`，本 task 只负责 JS heap 常驻量的 CSS/组件侧**。

## 方案（当前方案 = 精简守现状）

### A. 消灭常驻 animation tick（CPU 主线）

1. **流光边框改实现**（[03] 已拍板方向）：`@property --flow-ang` + `@keyframes` → 静态 conic 层 + `transform: rotate()`。transform 走合成器，渐变只光栅化一次。
   - **已知风险**：1px 边环靠 `mask-composite: exclude` 做，旋转带 mask 的方形层会破坏与 `border-radius` 的贴合。可能需拆「外层固定 mask + 内层旋转渐变」两层。**实施前先做视觉比对**（红线 3）。
2. **`bgShimmer 32s`**：32s 周期的背景微动，视觉收益近零、CPU 常驻。判 **删**（待 grill 与用户确认视觉取舍）。
3. **`pulseGlow` 动 `box-shadow`** → 改 `opacity` 或 `transform` 的 compositor-only 等价实现；`ProxyStatusSection.tsx:27` 的内联 animation 挪进 CSS 类（顺带纳入 reduced-motion 网）。
4. **skeleton / progressStripes**：仅在骨架/进度条**可见时**才应该 tick。判「保留但确保不可见时不挂载」，非删。
5. **reduced-motion 补全**：`globals.css:135-137` 与 `:923-926` 两块合并/补全，覆盖全部动画选择器。**内联 animation 一律迁出**（否则永远漏网）。

### B. 压 WebKit malloc 82MB → ≤32MB

`.glass::after` 的 116+ 离屏 buffer 是最大嫌疑（成本在**伪元素本身**，不在 animation）。手段按顺序：
1. 把 `.glass::after` 的 `mask` + `conic-gradient` 收进 `:hover`（[03] 已试，graphics 字节不变，但**当时未量 WebKit malloc**——本 task 补量这一维）
2. 收敛 `backdrop-filter` 用量：`.input`（47 处）与 `.btn`（12 处）是否真需要 blur —— 小控件上 12px blur 视觉收益低、每个都强制独立层

**量测口径硬约束**：只认 `footprint` 的 `phys_footprint` 与 `heap` 块数；**禁同进程内改 CSS 做 A/B**（[03] 已证不可靠：首轮就因 GPU 进程未吃满而作废）。每档独立重启 + 等满稳态。

## 为什么不选别的

| 备选 | 否决理由 |
|---|---|
| 全局关动画（`prefers-reduced-motion` 强制） | 视觉降级，压红线 3；且用户默认不开该设置 |
| 换掉 Liquid Glass 主题 | 功能/视觉删减，四条红线已排除 |
| 降动画帧率（如 30fps） | [03] 拍板要求**消灭**而非降频（目标 <0.5%） |
| React 层 memo / 虚拟化 | 50.2% CPU 归因明确落在 style 重算 + 光栅化，非 React 渲染 |

## 数据流（验证链路）

```
每档独立重启 app（release）→ 等满稳态（≥10min）
  → measure.sh 采 phys_footprint / graphics / WebKit malloc
  → sample 采空闲前台 CPU%（目标 <0.5%）
  → 视觉比对截图（改前/改后同尺寸同页），红线 3 判据
```

## 可能性分支（不进当前方案，仅留痕）

- **CSS `content-visibility: auto`** — 触发条件：若非可视区域元素仍占合成层。可能与 sticky/锚点导航冲突，需实测。
- **主题级「性能模式」开关** — 触发条件：若视觉与性能确实不可兼得且用户不愿降视觉。代价是两套 CSS 需并行维护 + i18n + 设置项，YAGNI。
- **`will-change` 全面清理** — 触发条件：若 `globals.css:125` 那一处被证明是层数放大器。当前仅一处，量级不足以单独立项。

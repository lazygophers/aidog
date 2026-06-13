# AiDog Logo 提示词设计（v2 · 无犬意象）

> 产出：可直接用于图像生成工具的 logo 提示词。所有风格关键词均映射自 `globals.css` / `liquidGlass.ts`，非臆造。
> v2 变更：去除犬/动物意象，改用抽象几何隐喻（棱镜折射 / 字母标 / 网关门户）表达「协议转换 + 平台聚合 + 路由」。

---

## 一、设计依据（代码提取）

| 维度 | 值 | 来源 |
|---|---|---|
| 设计语言 | Liquid Glass（Apple Vision Pro / macOS Tahoe） | `liquidGlass.ts:3-9` |
| 质感 | 多层半透明毛玻璃 + 内发光折射边缘 + 深度阴影 + 渐变 accent | `liquidGlass.ts:5-9` |
| 主色 | accent `#4A9EFF`(dark) / `#007AFF`(light)，渐变至 `#6BB3FF` | `liquidGlass.ts:49-50` |
| 背景 | 极深近黑 `#0a0a0c`(dark) / `#f0f0f3`(light) | `liquidGlass.ts:41,15` |
| 玻璃参数 | backdrop blur 24px · saturate 1.6–1.8 | `liquidGlass.ts:57,31-32` |
| 圆角 | squircle，radius-lg 20px / xl 28px | `liquidGlass.ts:36-37` |
| 折射高光 | inset 顶部 1px 亮线 `rgba(255,255,255,0.08)` | `globals.css:95` |
| 渐变方向 | 135° 对角 | `globals.css:201` |
| 语义色 | success `#34c759` 绿 / accent 蓝 / danger 红 | `globals.css:25-30,154` |

---

## 二、APP 定位与视觉隐喻（无犬）

- **名**：AiDog
- **功能**：AI API 网关/代理 — 多协议（OpenAI/Anthropic/Gemini）转换 + 平台聚合 + 分组路由 + 余额/配额守护
- **平台**：Tauri 桌面 app（macOS tray + 窗口），需 16px→1024px 全尺寸可辨识

**视觉隐喻映射**（用抽象几何表达功能，不依赖具象动物）：

| 功能 | 抽象隐喻 |
|---|---|
| 协议转换 | **棱镜折射** — 光（AI 请求）经玻璃棱镜折射成光谱（多协议输出） |
| 多平台聚合 | 多束光/数据流汇聚为一个发光节点；多层玻璃叠合 |
| 网关/路由 | 门户/拱门（gateway 入口）；分支节点 |
| 智能内核 | 发光光核 / 渐变蓝辐射 |
| 守护（可选） | 盾形外轮廓 |

---

## 三、Logo 概念方向（3 选 1，主推 A）

### A. 玻璃棱镜/折射光核（主推）⭐
一块多面液态玻璃棱镜，蓝色光束射入折射出内部光辉。
- 语义：光=AI请求流量，棱镜=AiDog网关，折射光谱=协议转换。三层含义自洽。
- 材质天然 Liquid Glass，风格零违和。
- 风险：三视图稍复杂，需控制几何简洁度。

### B. 字母标 "A"
AiDog 首字母几何化，渐变玻璃。
- 优点：品牌识别最直接、最简洁、app icon 最安全的形式。
- 风险：概念深度弱于 A。

### C. 网关门户/拱门
拱形门户，玻璃质感，内部蓝光。
- 语义：gateway 入口 / 路由枢纽。
- 风险：易和「门/窗」app 撞概念。

---

## 四、主提示词（方案 A · 玻璃棱镜）

### 4.1 Midjourney v6（推荐）

```
A minimalist premium app icon, an abstract geometric glass prism viewed at a subtle three-quarter angle, crafted from translucent frosted liquid glass with deep internal refraction, a focused beam of electric blue light entering one facet and refracting into a soft luminous gradient inside, core color transitioning from #4A9EFF to #6BB3FF, gentle inner light emission, layered semi-transparent glass panels with real depth and parallax, bright refractive highlight line along the top edge, Apple Vision Pro Liquid Glass aesthetic, volumetric soft shadows, deep near-black background #0a0a0c, clean elegant silhouette readable at small sizes, centered composition, rounded squircle canvas, no text no letters no words, 3D octane render, cinematic volumetric lighting, ultra detailed, modern premium --ar 1:1 --style raw --v 6
```

### 4.2 DALL·E 3 / 自然语言版

```
Design a minimalist premium macOS app icon on a 1:1 rounded-square (squircle) canvas. The subject is an abstract geometric glass prism seen at a subtle three-quarter angle, made entirely of translucent frosted liquid glass — the Apple Vision Pro "Liquid Glass" material — with multiple layered semi-transparent panels giving real depth. A focused beam of electric-blue light enters one facet and refracts into a soft luminous gradient inside the glass, the color transitioning smoothly from #4A9EFF to #6BB3FF. A bright refractive highlight runs along the top edge. The background is a deep near-black (#0a0a0c) with soft volumetric shadows. The silhouette must be clean and recognizable even at 16×16 pixels. No text, no letters, no animals. High-end 3D render with cinematic soft lighting, elegant and modern.
```

### 4.3 中文释义（供校对意图）

> 一枚极简高端的 macOS 应用图标，正方形圆角（squircle）画布。主体是一块抽象几何玻璃棱镜，呈微侧三分之二视角，整体由半透明磨砂液态玻璃（Apple Vision Pro「液态玻璃」材质）构成，多层半透明玻璃片叠加出真实纵深。一束聚焦的电蓝色光从一面射入，在玻璃内部折射成柔和发光的渐变，颜色从 `#4A9EFF` 平滑过渡到 `#6BB3FF`。顶部边缘有一道明亮的折射高光线。背景为极深近黑（`#0a0a0c`），下方投出柔和体积阴影。轮廓须干净、16×16 像素也能辨识。无文字、无字母、无动物。高端 3D 渲染，电影感柔光，优雅现代。

---

## 五、变体提示词

### 5.1 方案 B — 字母标 "A"（Midjourney）

```
A minimalist premium app icon, a bold geometric lettermark of the letter "A", crafted from translucent frosted liquid glass with a glowing electric blue gradient core #4A9EFF to #6BB3FF, inner light emission, refractive top highlight, Apple Vision Pro Liquid Glass aesthetic, layered glass depth, deep near-black background #0a0a0c, soft volumetric shadows, clean confident silhouette, centered, squircle canvas, no other text, 3D octane render, elegant modern --ar 1:1 --style raw --v 6
```

### 5.2 方案 C — 网关门户（Midjourney）

```
A minimalist premium app icon, an abstract arched gateway portal, crafted from translucent frosted liquid glass, glowing electric blue gradient light radiating from within the opening #4A9EFF to #6BB3FF, inner light emission, refractive edge highlights, Apple Vision Pro Liquid Glass aesthetic, layered semi-transparent glass depth, deep near-black background #0a0a0c, volumetric soft shadows, clean elegant silhouette readable at small sizes, centered, squircle canvas, no text, 3D octane render, modern premium --ar 1:1 --style raw --v 6
```

### 5.3 浅色背景变体（适配 light 主题 dock）

主提示词中把 `deep near-black background #0a0a0c` 替换为：
```
soft light gray background #f0f0f3, glass panels in cool white tones, same blue glowing core
```

---

## 六、Negative Prompt（强化排除动物/文字）

```
text, words, letters, typography, watermark, signature, dog, animal, canine, pet, mascot character, face, eyes, busy background, photorealistic, cluttered, low contrast, flat 2d colors, multiple subjects, human figure, harsh sharp edges, square corners, gradient background mesh, noise, grain, cartoon outline
```

> 已加入 `dog, animal, canine, pet, mascot character, face, eyes` —— 确保不出任何动物/具象角色，结果锁定在抽象几何。

---

## 七、使用建议

### 7.1 推荐工具与参数
| 工具 | 推荐配置 |
|---|---|
| **Midjourney v6** | `--ar 1:1 --style raw --v 6`（--style raw 减少艺术滤镜，更贴近描述） |
| **DALL·E 3** | 用 4.2 自然语言版 |
| **Ideogram** | 字母标方案 B 的首选（文字/字母渲染强） |

### 7.2 生成后处理
1. 选定方案后，用放大工具（Topaz / Real-ESRGAN）放大到 **1024×1024**。
2. 用 Tauri 官方命令生成全套图标：
   ```bash
   yarn tauri icon path/to/logo-1024.png
   ```
   自动产出 `icon.png / icon.icns / icon.ico / 32x32 / 128x128 / Square* / StoreLogo` 到 `src-tauri/icons/`。
3. 替换后 `yarn tauri build` 验证打包图标。

### 7.3 macOS Tray 单独适配（重要）
tray 图标在 16–22px 显示，液态玻璃质感在该尺寸会糊成一团。**建议额外生成一版单色剪影**：
```
A monochrome silhouette app icon template, single abstract prism shape, flat solid white shape on transparent background, minimal clean vector, no gradients no glass no 3D, high contrast, stencil style, suitable for macOS menu bar --ar 1:1 --style raw --v 6
```
macOS tray 自动按系统主题反色，故用单色 template 图标（`.set_template_image(true)`）。

### 7.4 风格自检（对照 prd 验收）
- [ ] 主色含 `#4A9EFF`→`#6BB3FF` 渐变
- [ ] 玻璃通透、有折射高光、有纵深
- [ ] 16×16 缩放后轮廓仍可辨识
- [ ] 深底（`#0a0a0c`）与浅底（`#f0f0f3`）均不违和
- [ ] 无文字字母、无动物犬类
- [ ] squircle 圆角，非直角

---

## 八、迭代关键词（微调用）

效果不满意时，按方向替换关键词：

| 想要的效果 | 加入/替换 |
|---|---|
| 更通透 | `higher translucency, more subsurface scattering, thinner glass` |
| 更深邃 | `darker background #050507, deeper shadows, stronger rim light` |
| 更极简 | `fewer glass layers, single emblem, more negative space, fewer facets` |
| 更科技 | `circuit pattern faintly visible inside glass, holographic edge, data stream lines` |
| 棱镜太复杂→更简洁 | `single triangular prism, fewer facets, flatter geometry` |
| 想要聚合感 | `multiple light beams converging into one glowing glass node` |

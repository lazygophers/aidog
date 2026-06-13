# AiDog Logo 提示词设计

> 产出：可直接用于图像生成工具的 logo 提示词。所有风格关键词均映射自 `globals.css` / `liquidGlass.ts`，非臆造。

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

## 二、APP 定位与意象

- **名**：AiDog（AI + Dog）
- **功能**：AI API 网关/代理 — 多协议（OpenAI/Anthropic/Gemini）转换 + 平台聚合 + 分组路由 + 余额/配额守护
- **意象关键词**：忠诚守卫 · 智能伙伴 · 流量看门狗 · 引导（导盲犬）· 敏捷 · 可信赖
- **平台**：Tauri 桌面 app（macOS tray + 窗口），需 16px→1024px 全尺寸可辨识

---

## 三、Logo 概念方向（3 选 1，主推 A）

### A. 玻璃犬徽（主推）⭐
抽象几何犬头正面，液态玻璃材质，内蕴蓝色光辉。
- 优点：简洁、小尺寸可辨识、主题中性、品牌识别清晰。
- 语义：AI 守护者 / 智能内核。

### B. 盾犬融合
盾牌轮廓 + 顶部犬首，玻璃质感。
- 优点：强化「守卫 AI 网关 / 守护配额」语义。
- 风险：细节多，小尺寸需简化。

### C. 字母标 A+犬耳
字母 "A" 顶端衍生犬耳，渐变玻璃。
- 优点：品牌字母识别强。
- 风险：app icon 通用准则建议少用文字/字母。

---

## 四、主提示词（方案 A）

### 4.1 Midjourney v6（推荐）

```
A minimalist premium app icon, a stylized geometric dog head emblem viewed front-on, symmetrical, crafted from translucent frosted liquid glass material with internal refraction and layered depth, glowing electric blue gradient core radiating from within, color transitioning from #4A9EFF to #6BB3FF, soft inner light emission, subtle refractive highlight line along the top edge, Apple Vision Pro Liquid Glass aesthetic, multiple semi-transparent glass panels stacked with depth and parallax, volumetric soft shadows, deep near-black background #0a0a0c, clean confident silhouette highly readable at small sizes, centered composition, large rounded squircle canvas, no text no letters no words, 3D octane render, cinematic volumetric lighting, ultra detailed, elegant modern --ar 1:1 --style raw --v 6
```

### 4.2 DALL·E 3 / 自然语言版

```
Design a minimalist premium macOS app icon on a 1:1 rounded-square (squircle) canvas. The subject is a stylized geometric dog head viewed from the front, symmetrical and confident. It is made entirely of translucent frosted liquid glass — the Apple Vision Pro "Liquid Glass" material — with multiple layered semi-transparent panels giving real depth and parallax. Inside the glass, a glowing electric-blue gradient core radiates light, transitioning smoothly from #4A9EFF to #6BB3FF. A subtle bright refractive highlight runs along the top edge of the glass. The background is a deep near-black (#0a0a0c) with soft volumetric shadows beneath the emblem. The silhouette must be clean and instantly recognizable even at 16×16 pixels. No text, no letters, no words. Render as a high-end 3D icon with cinematic soft lighting, elegant and modern.
```

### 4.3 中文释义（供校对意图）

> 一枚极简高端的 macOS 应用图标，正方形圆角（squircle）画布。主体是一只正面、对称、自信的风格化几何犬头。整体由半透明磨砂液态玻璃（Apple Vision Pro「液态玻璃」材质）构成，多层半透明玻璃片叠加出真实纵深与视差。玻璃内部有一团发光的电蓝色渐变光核，颜色从 `#4A9EFF` 平滑过渡到 `#6BB3FF`。玻璃顶部有一道细微的折射高光线。背景为极深近黑（`#0a0a0c`），图标下方投出柔和的体积阴影。轮廓须干净、即使在 16×16 像素也能一眼辨识。无文字、无字母。高端 3D 渲染，电影感柔光，优雅现代。

---

## 五、变体提示词

### 5.1 方案 B — 盾犬融合（Midjourney）

```
A minimalist premium app icon, a heraldic shield silhouette whose top edge merges seamlessly into a minimalist dog head crest, crafted from translucent frosted liquid glass, glowing electric blue gradient core #4A9EFF to #6BB3FF, inner light emission, refractive top highlight, Apple Vision Pro Liquid Glass aesthetic, layered semi-transparent glass depth, deep near-black background #0a0a0c, volumetric soft shadows, clean readable silhouette, squircle canvas, centered, no text, 3D octane render, elegant --ar 1:1 --style raw --v 6
```

### 5.2 方案 C — 字母标 A+犬耳（Midjourney）

```
A minimalist premium app icon, a lettermark of the letter "A" whose apex extends upward into two pointed dog ears, crafted from translucent frosted liquid glass with a glowing electric blue gradient #4A9EFF to #6BB3FF, inner light, refractive top highlight, Apple Vision Pro Liquid Glass aesthetic, layered glass depth, deep near-black background #0a0a0c, soft volumetric shadows, clean silhouette, squircle canvas, centered, bold geometric, 3D octane render, elegant modern --ar 1:1 --style raw --v 6
```

### 5.3 浅色背景变体（适配 light 主题 dock）

主提示词中把 `deep near-black background #0a0a0c` 替换为：
```
soft light gray background #f0f0f3, glass panels in cool white tones, same blue glowing core
```

---

## 六、Negative Prompt（通用）

```
text, words, letters, typography, watermark, signature, logo text, busy background, photorealistic fur, realistic animal photo, cluttered, low contrast, flat 2d colors, multiple subjects, human figure, harsh sharp edges, square corners, gradient background mesh, noise, grain, cartoon outline, mascot with eyes and mouth, duplicated shapes
```

> 注：排除「cartoon outline / mascot with eyes」是为了让结果偏抽象几何徽章而非具象卡通狗，更贴合高端 Liquid Glass 质感。若你想要亲切吉祥物风格，删除这两条。

---

## 七、使用建议

### 7.1 推荐工具与参数
| 工具 | 推荐配置 |
|---|---|
| **Midjourney v6** | `--ar 1:1 --style raw --v 6`（--style raw 减少艺术滤镜，更贴近描述） |
| **DALL·E 3** | 用 4.2 自然语言版，选「竖向/方形」 |
| **Ideogram** | 字母标方案 C 的首选（文字渲染强），其余亦可 |

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
A monochrome silhouette app icon template, single dog head emblem, flat solid white shape on transparent background, minimal clean vector, no gradients no glass no 3D, high contrast, stencil style, suitable for macOS menu bar --ar 1:1 --style raw --v 6
```
macOS tray 自动按系统主题反色，故用单色 template 图标（`.set_template_image(true)`）。

### 7.4 风格自检（对照 prd 验收）
- [ ] 主色含 `#4A9EFF`→`#6BB3FF` 渐变
- [ ] 玻璃通透、有折射高光、有纵深
- [ ] 16×16 缩放后轮廓仍可辨识
- [ ] 深底（`#0a0a0c`）与浅底（`#f0f0f3`）均不违和
- [ ] 无文字字母
- [ ] squircle 圆角，非直角

---

## 八、迭代关键词（微调用）

效果不满意时，按方向替换关键词：

| 想要的效果 | 加入/替换 |
|---|---|
| 更通透 | `higher translucency, more subsurface scattering, thinner glass` |
| 更深邃 | `darker background #050507, deeper shadows, stronger rim light` |
| 更亲和（吉祥物） | 删 negative 中 mascot 项 + 加 `friendly, rounded geometric dog, subtle eyes` |
| 更科技 | `circuit pattern faintly visible inside glass, holographic edge` |
| 更极简 | `fewer glass layers, single emblem, more negative space` |

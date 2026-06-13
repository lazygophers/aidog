# AiDog Logo 提示词设计（v3 · 扁平负空间）

> **v3 变更**：推翻 v1/v2 的 Liquid Glass（棱镜/字母标/门户）方向。logo 走 **扁平纯色 · 无透明 · 无光晕 · 无阴影 · 无 3D · 无狗**，独立于 UI 的 Liquid Glass 风格。用户选定方向：**负空间禅意派（原研哉 / Kenya Hara · Muji）**。

---

## 一、设计依据

| 维度 | 值 | 来源 |
|---|---|---|
| 设计哲学 | 扁平纯色负空间极简（Kenya Hara / Muji） | 用户选定方向 3 |
| 形体 | 单一实心几何块 + 精确切口负空间 | 负空间禅意核心手法 |
| 主色 | `#4A9EFF`（品牌 accent，纯色填充） | `liquidGlass.ts:49` |
| 背景 | `#f0f0f3`（浅底，light bg-base） | `liquidGlass.ts:15` |
| 质感 | **纯扁平**：零渐变 / 零阴影 / 零光晕 / 零透明 / 零 3D | 用户硬约束 |
| 圆角 | squircle（app icon 标准） | Tauri icon 规范 |

> **风格解耦说明**：logo 扁平 ≠ UI 扁平。UI 仍 Liquid Glass（`globals.css`）。app icon 与 UI 风格解耦是常见做法。仅保留品牌色 `#4A9EFF` 维持视觉连贯。

---

## 二、APP 定位 + 隐喻（无狗）

- **AiDog**：AI API 网关/代理（协议转换 + 平台聚合 + 分组路由 + 配额监控）
- **隐喻**：网关 → 形体被精确切口，负空间形成「门 / 通道」 → 象征 AI 请求穿越的网关
- 全抽象几何，零犬关联

---

## 三、选定方向 · 负空间禅意（Kenya Hara）

- **主体**：一块纯色 `#4A9EFF` 实心圆角方块
- **负空间**：正中精确切出竖向网关开口（高窄圆角缝隙），形成通道意象
- **哲学**：Muji 式留白，少即是多，概念由负空间承载
- **气质**：安静、克制、概念性、优雅

---

## 四、主提示词

### 4.1 Midjourney v6

```
Flat minimalist app icon, a single solid geometric form — a rounded square filled in pure flat solid color #4A9EFF — with a precise clean vertical gateway aperture cut through its center forming negative space that reads as a passage or gateway, symbolizing an AI gateway that requests pass through, solid flat light gray background #f0f0f3, maximum negative space and breathing room, Kenya Hara Muji-inspired minimalism, absolutely no gradients no shadows no glow no transparency no 3D effects, crisp hard vector edges, conceptual elegant quiet, centered, squircle canvas, flat vector design --ar 1:1 --style raw --v 6
```

### 4.2 DALL·E 3 / 自然语言版

```
Design a flat minimalist app icon on a 1:1 rounded-square (squircle) canvas. The subject is a single solid rounded square filled in one pure flat color #4A9EFF. Cut precisely through its center is a clean vertical gateway aperture — a tall narrow rounded opening — forming negative space that reads as a gateway or passage, symbolizing an AI gateway that requests pass through. The background is a solid flat light gray #f0f0f3. Use maximum negative space and breathing room, in the spirit of Kenya Hara and Muji minimalism. Absolutely no gradients, no shadows, no glow, no transparency, no 3D effects — pure flat solid color blocks with crisp hard vector edges. Must be recognizable even at 16×16 pixels. Elegant, conceptual, quiet.
```

### 4.3 中文释义（校对意图）

> 一枚扁平极简的 app 图标。主体是一块纯色 `#4A9EFF` 实心圆角方块，其正中精确切出一道竖向网关开口（高窄圆角缝隙），形成负空间——读作一扇门/通道，象征 AI 请求穿越的网关。背景为纯色浅灰 `#f0f0f3`。大量留白，原研哉/Muji 式极简。绝对无渐变、无阴影、无光晕、无透明、无 3D——纯色块、硬边矢量。16×16 也能辨识。优雅、克制、安静。

---

## 五、Negative Prompt

```
gradient, shadow, glow, transparency, glass, blur, 3D, depth, reflection, bevel, emboss, texture, noise, grain, dog, animal, canine, mascot, face, text, letters, words, busy, cluttered, multiple colors, photographic, realistic, skeuomorphic
```

> 重点排除 gradient/shadow/glow/transparency/3D（锁死扁平）+ dog/animal（锁死无犬）+ 多色（锁死单色块）。

---

## 六、尺寸 / 适配

### 6.1 主图标
1024×1024 生成 → `yarn tauri icon logo-1024.png` 自动产出全套（`icon.png / icon.icns / icon.ico / 32x32 / 128x128 / Square* / StoreLogo`）到 `src-tauri/icons/`。

### 6.2 深底变体（dark dock）
主提示词替换：
```
background #0a0a0c, the rounded square form filled in solid white #ffffff
```

### 6.3 macOS Tray
负空间 logo 本身极简单色，**直接做单色 template**：
```
Monochrome silhouette app icon template, single rounded square with vertical gateway aperture cut-out, flat solid white shape on transparent background, no gradients no effects, high contrast stencil style, suitable for macOS menu bar --ar 1:1 --style raw --v 6
```
`.set_template_image(true)` 让系统按主题反色。16px 清晰。

---

## 七、迭代关键词（微调）

| 想要的效果 | 替换/加入 |
|---|---|
| 深底版 | `background #0a0a0c, form in solid white #ffffff` |
| 开口变形 | `horizontal aperture` / `circular aperture` / `keyhole cut-out` |
| 形体变形 | `solid circle` / `solid hexagon`（替 rounded square） |
| 更克制 | `more negative space, thinner aperture, smaller form` |
| 双色（克制） | 形体 `#4A9EFF`，开口边缘描一道 `#6BB3FF`（仍扁平无光） |
| 更概念 | `the aperture forms a subtle secondary shape, optical illusion negative space` |

---

## 八、与 v1/v2 的关系

- **v1**（已废弃）：犬徽 + Liquid Glass — 用户否决「不要狗头」
- **v2**（已废弃）：棱镜/字母标/门户 + Liquid Glass — 用户否决「不要透明、不要光晕、要扁平」+「完全重新设计」
- **v3**（当前）：扁平负空间禅意 — 满足全部硬约束（扁平/无透明/无光晕/无狗）

历史 prompt 不再可用，以 v3 为准。

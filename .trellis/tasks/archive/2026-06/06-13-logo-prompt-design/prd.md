# AiDog APP Logo 提示词设计

## 目标
产出完善、可直接用于图像生成工具（Midjourney / DALL·E / Ideogram）的 logo 提示词，生成结果须符合 AiDog 当前 Liquid Glass 设计语言。

## 背景
- **APP**：AiDog — AI API 网关/代理（OpenAI/Anthropic/Gemini 协议转换 + 平台聚合 + 分组路由 + 余额/配额守护）。Tauri 桌面 app，macOS 优先（tray + 窗口）。
- **当前图标**：`src-tauri/icons/` 为 Tauri 默认图标集（icon.png/icns/ico/Square*/StoreLogo），未品牌化。
- **设计风格来源**：`src/styles/globals.css` + `src/themes/liquidGlass.ts`（5 主题：liquidGlass 默认 + catppuccin/dracula/nord/solarized）。

## 设计约束（从代码提取，提示词必须对齐）
| 维度 | 值 |
|---|---|
| 设计语言 | Liquid Glass（Apple Vision Pro / macOS Tahoe） |
| 质感 | 多层半透明毛玻璃 + 内发光折射边缘 + 深度阴影 + 渐变 accent |
| 主色 | accent `#4A9EFF`(dark)/`#007AFF`(light) → 渐变 `#6BB3FF` |
| 背景 | 极深 `#0a0a0c`(dark) / `#f0f0f3`(light) |
| 玻璃 | blur 24px · saturate 1.6–1.8 |
| 圆角 | squircle，radius 20–28px |
| 折射 | inset 顶部 1px 高光线 |

## 交付
- `design.md` — 设计分析 + 3 概念方向 + 主提示词(英) + 变体 + negative + 使用建议。

## 验收
- [ ] 提示词可直接粘贴 MJ/DALL·E 使用
- [ ] 风格关键词全部映射到代码提取值（非臆造）
- [ ] 小尺寸(tray 16–22px)可辨识
- [ ] 主题中性（深/浅底均可用）
- [ ] 含 negative prompt + 多工具适配说明

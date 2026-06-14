# PRD: 3 palette label 多语言补全

## 目标

gruvbox / rosePine / tokyoNight 3 个 palette 在 8 locale 全为 verbatim 品牌名 (无本地化描述), 与 nord→"北欧冷色" / dracula→"德古拉暗紫" / catppuccin→"卡布奇诺柔色" 模式不一致。补本地化描述, 风格 = 纯本地化描述 (同 nord 模式, 用户确认)。

## 现状锚点

- `src/locales/{zh-CN,en-US,ar-SA,fr-FR,de-DE,ru-RU,ja-JP,es-ES}.json` 的 `theme.color.{gruvbox,rosePine,tokyoNight}` 8×3=24 处均为英文品牌名 verbatim。
- 参考: `theme.color.nord` 各 locale 有本地化 (北欧冷色/Nordic Cool/نورد البارد/Nordique froid/Nordisch kühl/Норд холодный/ノード寒色/Nórdico frío)。
- `src/components/Sidebar.tsx` 用 `t(\`theme.color.${themeColor}\`)` 查 label, key 不变只改 value。

## 本地化方案 (3 palette × 8 locale = 24 value)

### gruvbox (复古暖棕 — groove+box 暖色调复古终端色板)
| locale | value |
|---|---|
| zh-CN | 复古暖棕 |
| en-US | Retro Warm |
| ar-SA | كلاسيكي دافئ |
| fr-FR | Rétro Chaud |
| de-DE | Retro Warm |
| ru-RU | Ретро Тёплый |
| ja-JP | レトロ暖色 |
| es-ES | Retro Cálido |

### rosePine (玫瑰松 — Rosé 玫瑰酒色 + Pine 松, 柔粉紫灰)
| locale | value |
|---|---|
| zh-CN | 玫瑰松柔 |
| en-US | Rosy Pine |
| ar-SA | وردة الصنوبر |
| fr-FR | Rose Pin |
| de-DE | Rosenkiefer |
| ru-RU | Розовая Сосна |
| ja-JP | ローズパイン |
| es-ES | Rosa Pino |

### tokyoNight (东京夜蓝 — 深蓝青霓虹都市夜色)
| locale | value |
|---|---|
| zh-CN | 东京夜蓝 |
| en-US | Tokyo Night |
| ar-SA | ليل طوكيو |
| fr-FR | Nuit de Tokyo |
| de-DE | Tokio Nacht |
| ru-RU | Ночь Токио |
| ja-JP | 東京ナイト |
| es-ES | Noche de Tokio |

> en-US tokyoNight 保留 "Tokyo Night" (已是英文描述短语, 同 nord→"Nordic Cool" 翻译逻辑; tokyoNight 英文本身即描述)。

## 实施步骤

1. 8 locale 文件, 替换 `theme.color.{gruvbox,rosePine,tokyoNight}` value 按上表 (key 不变)。
2. python 脚本批量改 (key→value map), 保 json 格式 (indent=2, ensure_ascii=False)。

## 验收标准

- [ ] 8 locale × 3 key = 24 value 全替换为本地化描述 (非品牌名 verbatim)。
- [ ] `check-i18n.mjs` 零缺失 (key 未动)。
- [ ] `tsc --noEmit` 通过 (无 TS 改动, 确认无副作用)。
- [ ] grep 确认 3 key 仍在 (仅 value 变)。

## 风险

- **翻译不准**: 各语言意译主观。缓解: 参考 nord/dracula/catppuccin 既有翻译风格 (形容词+色/质), 音译优先于直译怪异处。
- **ar-SA RTL**: 阿拉伯语描述需正确 Unicode 方向, json 转义正常即可。

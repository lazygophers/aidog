# 主题名多语言 — 品牌名描述性翻译

## 背景
theme.* i18n keys 全 8 语言已有。theme.label / theme.liquidGlass / theme.light / theme.dark 已译。但 4 品牌名 (Nord/Dracula/Catppuccin/Solarized) 各语言保留英文原名。

用户要求：品牌名描述性翻译（如 Nord→北欧冷色，非音译）。

## 改动
仅改 4 个 i18n key 的值，8 语言各译描述性本地名（保留品牌辨识 + 加特征描述）：

| key | zh-CN | en-US | ar-SA | fr-FR | de-DE | ru-RU | ja-JP | es-ES |
|-----|-------|-------|-------|-------|-------|-------|-------|-------|
| theme.nord | 北欧冷色 | Nordic Cool | نورد البارد | Nordique froid | Nordisch kühl | Норд холодный | ノード寒色 | Nórdico frío |
| theme.dracula | 德古拉暗紫 | Dracula Dark | دراكولا الداكن | Dracula sombre | Dracula dunkel | Дракула тёмный | ドラキュラ闇 | Drácula oscuro |
| theme.catppuccin | 卡布奇诺柔色 | Catppuccin Soft | كابتشينو الناعم | Catppuccin doux | Catppuccin sanft | Капучино мягкий | カプチーノ柔 | Capuchino suave |
| theme.solarized | 日晒柔光 | Solarized Soft | شمسي الناعم | Solarisé doux | Solarisiert sanft | Solarized мягкий | ソラライズ柔 | Solarizado suave |

不动: theme.label / theme.liquidGlass (已译) / theme.light / theme.dark。

## 验证
- 8 语言 4 key 值非英文原名
- yarn build

# License badge/section 修正为 AGPL-3.0

## 背景
- 实际协议：`LICENSE` = GNU AGPL v3，`package.json` / `Cargo.toml` = `AGPL-3.0-or-later`。
- README badge 行（行 11）8 语言均标 `license-MIT-blue`，与实际不符。
- License section 正文：README.md (zh) 已正确写 AGPL-3.0 + 说明；其余 7 语言（en/fr/de/es/ar/ja/ru）仅写 `MIT`，错误。

## 改动（8 README，docs 无 license 内容不动）
1. **Badge 行**（行 11）：`license-MIT-blue` → `license-AGPL_3.0-blue`（shields.io `_` 渲染为空格）。
2. **License section 正文**（en/fr/de/es/ar/ja/ru）：把 `MIT` 替换为 `[GNU AGPL-3.0-or-later](LICENSE) © AiDog` + 本地化的 AGPL 网络服务开源说明（与 zh 风格一致）。

## 验证
- `grep -rn "license-MIT\|^MIT$"` 在 README 中 0 命中。
- badge 链接 `#license` anchor 保留。

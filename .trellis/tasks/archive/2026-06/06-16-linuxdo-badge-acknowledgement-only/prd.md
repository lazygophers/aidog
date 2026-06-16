# LINUX DO 徽章仅保留在致谢区

## 背景
- LINUX DO 徽章当前在 8 个 README + 8 个 docs index.mdx 的顶部 badge 行各出现一次；
- 8 个 README 另在底部致谢 section 已有一份 → 重复。
- 8 个 docs 无致谢 section，仅顶部一份。
- 用户要求：徽章只该在鸣谢/致谢处。

## 改动
1. **8 README**（zh/en/fr/de/es/ar/ja/ru）：删除顶部 badge 行的 LINUX DO 徽章（行 13），保留底部致谢 section 的徽章不变。
2. **8 docs/docs/<lang>/index.mdx**：删除顶部 badge 行的 LINUX DO 徽章（zh/en/fr/de/es/ar/ja 行 52；ru 行 6），在文件末尾（技术栈表后）追加致谢 section（标题 + 徽章 + 感谢文案，文案与对应 README 一致）。

## 致谢文案（docs 追加，复用 README 既有翻译）
- zh: `## 致谢` + 徽章 + `感谢 [LINUX DO](https://linux.do) 社区。`
- en: `## Acknowledgements` + 徽章 + `Thanks to the [LINUX DO](https://linux.do) community.`
- fr/de/es/ar/ja/ru 同 README 既有文案。

## 验证
- `grep -rn "ld-badge"`：每文件恰好 1 次（README 在致谢，docs 在新增致谢）。
- 无代码改动，无需 cargo/test。

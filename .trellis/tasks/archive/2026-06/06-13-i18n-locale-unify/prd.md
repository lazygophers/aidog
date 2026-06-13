# i18n 多端 locale 统一 + 缺失补全

## 背景
三端 locale 集合互不一致 + 前端 JSON key 不齐 + docs 孤儿目录。

## 现状（事实）
- 前端 locales (`src/locales/`): zh-CN en-US ar-SA fr-FR de-DE ru-RU ja-JP (7，**无 es**)
- docs rspress locales (`docs/rspress.config.ts` + `docs/i18n.json`): zh en ja fr de ar es (7，**无 ru**)
- docs/docs/ru/ 孤儿目录（rspress 未配 ru，仅 api/api-reference.mdx + _nav.json，2 文件残缺）
- README: zh(.md 默认) en ar es fr ru (6，**无 ja de**)
- 前端 JSON key 数: zh 932 / en 813 / 其余 5 个均 755 → 非 en/zh locale 缺 58~177 key，切换后部分菜单/文案显示 raw key 或 fallback en
- docs es 35 mdx vs zh 36 mdx（缺 1）

## 目标（统一 8 语言全集：zh/en/ja/fr/de/ar/ru/es）
保留所有已有完整内容，只补缺不删除。

## 交付
1. **前端 +es**: 新增 `src/locales/es-ES.json`（对照 en-US 翻译）；`index.ts` 加类型 + ALL_LOCALES + lazyLoader
2. **前端 key 补齐**: ar/fr/de/ru/ja/es 6 个 locale 对齐 en-US 的 813 key（补缺失 key，值翻译）
3. **docs +ru 全套**: docs/docs/ru/ 补全对照 zh/en 的 9 章节全套 mdx + 各 _meta.json；`docs/i18n.json` 加 ru 词条；`docs/rspress.config.ts` locales 加 ru
4. **docs es 补缺 1 mdx**: 对齐 zh 36 篇
5. **README +ja +de**: 对照 README.en 翻译 README.ja.md + README.de.md
6. **清理**: docs/docs/ru 孤儿 _nav.json 与新内容合并（不重复）

## 验证
- `yarn build` (tsc + vite) 通过
- `cd docs && yarn build` 通过（rspress 全 8 locale）
- 前端: `python3` 脚本校验 7 locale JSON key 集合 ⊇ en-US key 集合
- docs: 8 locale mdx 文件数对齐（zh 为基准）

## 不含（P1 后续 task）
- 前端 tsx 硬编码中文 i18n 化（editors.tsx 2826 / Platforms 2266 等，规模过大，独立任务）

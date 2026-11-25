# docs 站与多语言 README：图片路径、断链门禁、译本对齐

## rspress 图片必须放 docs/docs/public/

rspress 静态资源根是 `docs/docs/public/`，md 里 `/xxx.png` 引用解析到它。图片放 `docs/public/`（少一层）构建找不到，`yarn check:docs` 的 rspress build 报 Dead image 红。README 与 docs 两用时落两份：仓库根 `screenshots/`（README 相对路径引用）+ 拷贝进 `docs/docs/public/`（docs 页 `/` 引用）；三处图片目录（assets / screenshots / docs/docs/public）都受 `scripts/check-docs-drift.mjs` 的 imgDirs 盘点。

## 交付门禁

改 docs 或任一 README 后跑：

1. `yarn check:docs`（gen-command-docs --check + docs workspace eslint / biome / rspress build——Dead image、断链图片都在这一步红）；
2. `yarn check:docs-drift`；
3. 相对路径断链用脚本核（参照 `.tmp/docs-final-commit/zh-link-check.mjs`：walk 目录 → 正则提取 `](相对路径)` → `fs.existsSync` 逐条核；站内 `/` 开头路由链接与 http 外链跳过）。README 图片相对路径在 8 语言目录各解析一次，逐语言核对——zh 链接对不代表译本对。

## 多语言译本对齐

README/docs 译本以中文版为结构真值源：en docs 站目录与路由结构镜像 zh（整体重写为镜像结构，不保留 Rspress 模板旧目录）。批量产出译本后做一次「标题骨架比对」：提取各语言 `^#{1,3} ` 标题序列与中文版逐条对照——标题文字可译，顺序、层级、条数不可漂移；章节内容差异（版本/平台特有段）在译本内显式声明，不静默删段。机翻 agent 漏段 / 乱序靠这一步拦。

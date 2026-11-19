# docs 站与多语言 README：图片路径、断链门禁、译本对齐

2026-11-18 文档全面重写轮沉淀（commit db26961b6：8 README + 37 zh docs + en docs
站从 Rspress 模板重写为镜像中文结构；commit 0a4fc1f74：40 张真渲染截图嵌入 62 文件）。

## rspress 图片必须放 docs/docs/public/

rspress 的静态资源根是 `docs/docs/public/`，md 里 `/xxx.png` 引用解析到它。图片
放 `docs/public/`（少一层）构建找不到，`yarn check:docs` 的 rspress build 报
Dead image 红（本轮实锤）。README 与 docs 两用时落两份：仓库根 `screenshots/`
（README 相对路径引用）+ 拷贝进 `docs/docs/public/`（docs 页 `/` 引用）；三处
图片目录都受 `scripts/check-docs-drift.mjs` 的 imgDirs 盘点（assets /
screenshots / docs/docs/public）。

## 交付门禁

改 docs 或任一 README 后跑：

1. `yarn check:docs`（gen-command-docs --check + docs workspace eslint / biome /
   rspress build——Dead image、断链图片都在这一步红）；
2. `yarn check:docs-drift`；
3. 相对路径断链用脚本核（本轮实锤形态，`.tmp/docs-final-commit/zh-link-check.mjs`
   的写法：walk 目录 → 正则提取 `](相对路径)` → `fs.existsSync` 逐条核；站内
   `/` 开头路由链接与 http 外链跳过）。README 图片相对路径在 8 语言目录各解析
   一次，逐语言核对——zh 链接对不代表译本对。

## 多语言译本对齐

README/docs 译本以中文版为结构真值源：en docs 站的目录与路由结构镜像 zh（本轮
做法是整体重写为镜像结构，不保留 Rspress 模板旧目录）。批量产出译本后做一次
「标题骨架比对」：提取各语言 `^#{1,3} ` 标题序列，与中文版逐条对照——标题文字
可译，顺序、层级、条数不可漂移；章节内容差异（版本/平台特有段）在译本内显式
声明，不静默删段。机翻 agent 漏段 / 乱序靠这一步拦（人工逐页比对 37 页不可行）。

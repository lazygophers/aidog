# SKEIN core 规则索引 (章节粒度: 一行一条规则)

类目: build(5), i18n(7) · 关联见 [backlinks.md](backlinks.md)

| rule (topic.md#标题) | category | title | keywords | status/出链 | summary |
|---|---|---|---|---|---|
| build/shadcn-infra-02.md#MUST 迁移方式 | build | MUST 迁移方式 | tailwind,v4,preflight,migration,css | active | 1. 仅 import theme/utilities（跳过 preflight/base） 2. 或单行总导入：@im… |
| build/shadcn-infra-02.md#关联 | build | 关联 | tailwind,v4,preflight,migration,css | active / →shadcn-infra-28,shadcn-infra-30 | [[shadcn-infra-30]] [[shadcn-infra-28]] |
| build/shadcn-infra-02.md#硬约束 | build | 硬约束 | tailwind,v4,preflight,migration,css | active | Tailwind v4 迁移过程中**禁使用旧 v3 的三行导入方式**，必须用 v4 的 @import 方式。 |
| build/shadcn-infra-02.md#禁用的旧方式 | build | 禁用的旧方式 | tailwind,v4,preflight,migration,css | active | ❌ @tailwind base;  /* v3 方式，v4 崩盘 */ ❌ @tailwind components;… |
| build/shadcn-infra-02.md#适用 | build | 适用 | tailwind,v4,preflight,migration,css | active | Tailwind v3 → v4 迁移、新项目用 v4 |
| i18n/rule-04.md#MUST 硬约束 | i18n | MUST 硬约束 | i18n,locale,翻译,check-i18n,8语言,同步 | active | 新增 i18n key 必须同时补齐 8 个语言文件（zh-Hans/en-US/ar-SA/fr-FR/de-DE/r… |
| i18n/rule-04.md#关联 | i18n | 关联 | i18n,locale,翻译,check-i18n,8语言,同步 | active | i18n-flat-key-convention |
| i18n/rule-04.md#处理流程 | i18n | 处理流程 | i18n,locale,翻译,check-i18n,8语言,同步 | active | ```bash # 新增 key 后检查 yarn check-i18n  # 自动补齐（示例：从 zh-Hans 复制… |
| i18n/rule-04.md#案例 | i18n | 案例 | i18n,locale,翻译,check-i18n,8语言,同步 | active | - shadcn-pages m-checkfix：新增 3 key 同步补 8 locale（1db931fe） |
| i18n/rule-04.md#检查机制 | i18n | 检查机制 | i18n,locale,翻译,check-i18n,8语言,同步 | active | - `check-i18n` 守门：跑 `yarn check-i18n` 检测 key 同步 - 缺失语言会导致对应语… |
| i18n/rule-04.md#触发场景 | i18n | 触发场景 | i18n,locale,翻译,check-i18n,8语言,同步 | active | alert() 迁移到 toast() 等新 i18n 机制时，新增翻译 key 必须同步到所有 locale。 |
| i18n/rule-04.md#适用 | i18n | 适用 | i18n,locale,翻译,check-i18n,8语言,同步 | active | - 所有 i18n key 新增/修改 - alert() → toast() 迁移（如 shadcn-pages ta… |

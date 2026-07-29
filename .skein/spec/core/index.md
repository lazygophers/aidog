# SKEIN core 规则索引 (章节粒度: 一行一条规则)

类目: build(5), domain(8), i18n(7) · 关联见 [backlinks.md](backlinks.md)

| rule (topic.md#标题) | category | title | keywords | status/出链 | summary |
|---|---|---|---|---|---|
| build/shadcn-infra-02.md#MUST 迁移方式 | build | MUST 迁移方式 | tailwind,v4,preflight,migration,css | active | 1. 仅 import theme/utilities（跳过 preflight/base） 2. 或单行总导入：@im… |
| build/shadcn-infra-02.md#关联 | build | 关联 | tailwind,v4,preflight,migration,css | active / →shadcn-infra-28,shadcn-infra-30 | [[shadcn-infra-30]] [[shadcn-infra-28]] |
| build/shadcn-infra-02.md#硬约束 | build | 硬约束 | tailwind,v4,preflight,migration,css | active | Tailwind v4 迁移过程中**禁使用旧 v3 的三行导入方式**，必须用 v4 的 @import 方式。 |
| build/shadcn-infra-02.md#禁用的旧方式 | build | 禁用的旧方式 | tailwind,v4,preflight,migration,css | active | ❌ @tailwind base;  /* v3 方式，v4 崩盘 */ ❌ @tailwind components;… |
| build/shadcn-infra-02.md#适用 | build | 适用 | tailwind,v4,preflight,migration,css | active | Tailwind v3 → v4 迁移、新项目用 v4 |
| domain/rule-66.md#关联 | domain | 关联 | - | active / →bundled-models-fallback,time-tiers-apply-idiom | [[time-tiers-apply-idiom]] [[bundled-models-fallback]] |
| domain/rule-66.md#案例 | domain | 案例 | - | active | 原错 (billing.rs 未传参) → 日志字段时刻定价与当前时刻定价混杂 → 审计重放价格错 修后 → creat… |
| domain/rule-66.md#硬约束 | domain | 硬约束 | - | active | `resolve_price` 新增末位参数 `now_ms: i64`，调用点按用途选传值：  / 调用点 / 传值 … |
| domain/rule-66.md#禁用 | domain | 禁用 | - | active | ❌ 所有调用点统一传 0（会导致时段定价形同虚设） ❌ 测试传 `now()`（会让既有基准价断言失败） ❌ 签名改动后… |
| domain/rule-67.md#关联 | domain | 关联 | - | active / →rule-66,time-tiers-apply-idiom | [[rule-66]] [[time-tiers-apply-idiom]] |
| domain/rule-67.md#案例 | domain | 案例 | - | active | 原错 → estimate 的两处取价未乘 peak_hours·multiplier，而 calc_est_cost … |
| domain/rule-67.md#硬约束 | domain | 硬约束 | - | active | estimate 流程中**任一分支加 peak 倍率，对边必补**（既存 bug 根因）：  - `estimate/… |
| domain/rule-67.md#禁用 | domain | 禁用 | - | active | ❌ 仅余额扣减乘倍率，手动预算不乘（口径分裂：扣数 ≠ 前端显示） ❌ 仅某一段乘倍率，其他相关路径不补（隐性 bug，… |
| i18n/rule-04.md#MUST 硬约束 | i18n | MUST 硬约束 | i18n,locale,翻译,check-i18n,8语言,同步 | active | 新增 i18n key 必须同时补齐 8 个语言文件（zh-Hans/en-US/ar-SA/fr-FR/de-DE/r… |
| i18n/rule-04.md#关联 | i18n | 关联 | i18n,locale,翻译,check-i18n,8语言,同步 | active | i18n-flat-key-convention |
| i18n/rule-04.md#处理流程 | i18n | 处理流程 | i18n,locale,翻译,check-i18n,8语言,同步 | active | ```bash # 新增 key 后检查 yarn check-i18n  # 自动补齐（示例：从 zh-Hans 复制… |
| i18n/rule-04.md#案例 | i18n | 案例 | i18n,locale,翻译,check-i18n,8语言,同步 | active | - shadcn-pages m-checkfix：新增 3 key 同步补 8 locale（1db931fe） |
| i18n/rule-04.md#检查机制 | i18n | 检查机制 | i18n,locale,翻译,check-i18n,8语言,同步 | active | - `check-i18n` 守门：跑 `yarn check-i18n` 检测 key 同步 - 缺失语言会导致对应语… |
| i18n/rule-04.md#触发场景 | i18n | 触发场景 | i18n,locale,翻译,check-i18n,8语言,同步 | active | alert() 迁移到 toast() 等新 i18n 机制时，新增翻译 key 必须同步到所有 locale。 |
| i18n/rule-04.md#适用 | i18n | 适用 | i18n,locale,翻译,check-i18n,8语言,同步 | active | - 所有 i18n key 新增/修改 - alert() → toast() 迁移（如 shadcn-pages ta… |

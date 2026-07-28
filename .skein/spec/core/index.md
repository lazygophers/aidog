# SKEIN core 规则索引 (章节粒度: 一行一条规则)

类目: build(5), i18n(7), optimization(5), testing(5), ts-rust-boundary(10) · 关联见 [backlinks.md](backlinks.md)

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
| optimization/manual-budget-empty-shortcircuit.md#manual_budget 零配额短路：进写连接前预检 | optimization | manual_budget 零配额短路：进写连接前预检 | manual-budget,optimization,db-write,shortcircuit,loadgen | active | - |
| optimization/manual-budget-empty-shortcircuit.md#关键点 | optimization | 关键点 | manual-budget,optimization,db-write,shortcircuit,loadgen | active | - **硬约束**：配额存在时行为不变，短路仅对「零配额」分支生效 - **非 mock 专属**：真实转发路径共用同一… |
| optimization/manual-budget-empty-shortcircuit.md#方案 | optimization | 方案 | manual-budget,optimization,db-write,shortcircuit,loadgen | active | **分两阶段：**  1. **只读池预检**（`has_any_budget`，line:189-203）：用只读池（… |
| optimization/manual-budget-empty-shortcircuit.md#用途 | optimization | 用途 | manual-budget,optimization,db-write,shortcircuit,loadgen | active | 高频转发路径的每请求冷路径优化，减少单线程 DB 写锁争。适用于： - mock/真实平台混用的压测 - 用户未配额时的… |
| optimization/manual-budget-empty-shortcircuit.md#问题 | optimization | 问题 | manual-budget,optimization,db-write,shortcircuit,loadgen | active | `apply_manual_budgets`（`manual_budget.rs:211-246`）处理用户手动配额时，… |
| testing/deterministic-pseudorandom-loadgen.md#关键点 | testing | 关键点 | testing,loadgen,deterministic,pseudorandom,splitmix64,atomic,error_rate | active | - **确定性**：给定 error_rate 的序列完全由进程启动顺序决定，重复压测结果稳定 - **分布均匀**：s… |
| testing/deterministic-pseudorandom-loadgen.md#压测可复现的确定性伪随机（原子计数器+哈希） | testing | 压测可复现的确定性伪随机（原子计数器+哈希） | testing,loadgen,deterministic,pseudorandom,splitmix64,atomic,error_rate | active | - |
| testing/deterministic-pseudorandom-loadgen.md#方案 | testing | 方案 | testing,loadgen,deterministic,pseudorandom,splitmix64,atomic,error_rate | active | **进程级原子计数器 + 乘法哈希** (`proxy/mock.rs:2-16`)：  ```rust static … |
| testing/deterministic-pseudorandom-loadgen.md#用途 | testing | 用途 | testing,loadgen,deterministic,pseudorandom,splitmix64,atomic,error_rate | active | - mock 平台的 error_rate 注入 - 压测场景的确定性故障模拟 - 内存/CPU 基准测试（需要重复压测… |
| testing/deterministic-pseudorandom-loadgen.md#问题 | testing | 问题 | testing,loadgen,deterministic,pseudorandom,splitmix64,atomic,error_rate | active | 压测场景（尤其是性能/内存压测）需要可复现的伪随机行为，用于注入 `error_rate=0.05`（5% 请求返回 4… |
| ts-rust-boundary/mock-config-4layer-consistency.md#mock 配置四层覆盖的字段一致性检查 | ts-rust-boundary | mock 配置四层覆盖的字段一致性检查 | ts-rust-boundary,mock-config,consistency,serde,json-boundary | active | - |
| ts-rust-boundary/mock-config-4layer-consistency.md#失配场景 | ts-rust-boundary | 失配场景 | ts-rust-boundary,mock-config,consistency,serde,json-boundary | active | / 症状 / 原因 / /---/---/ / TS 编辑器赋值后无效 / `serializeMockConfig` … |
| ts-rust-boundary/mock-config-4layer-consistency.md#检查表（四处同步） | ts-rust-boundary | 检查表（四处同步） | ts-rust-boundary,mock-config,consistency,serde,json-boundary | active | ### 1. Rust struct 定义 (`config.rs:11-25`) - [ ] 新字段声明的类型：`Op… |
| ts-rust-boundary/mock-config-4layer-consistency.md#用途 | ts-rust-boundary | 用途 | ts-rust-boundary,mock-config,consistency,serde,json-boundary | active | Rust↔TS 跨边界的配置字段迭代通用检查表。适用于： - 平台/插件配置扩展 - 新增可选设置 - 配置升级 mig… |
| ts-rust-boundary/mock-config-4layer-consistency.md#问题 | ts-rust-boundary | 问题 | ts-rust-boundary,mock-config,consistency,serde,json-boundary | active | mock 配置在四层跨 Rust↔TS 边界流转，任一处字段定义/序列化不一致都导致静默失配：  1. **Rust s… |
| ts-rust-boundary/optional-config-backward-compat.md#Option<T> 可选字段的向后兼容方案 | ts-rust-boundary | Option<T> 可选字段的向后兼容方案 | ts-rust-boundary,option,backward-compat,unwrap_or,config-migration | active | - |
| ts-rust-boundary/optional-config-backward-compat.md#关键点 | ts-rust-boundary | 关键点 | ts-rust-boundary,option,backward-compat,unwrap_or,config-migration | active | - **旧字段保留**：必须保留兼容入口，不删不改 - **Option/undefined 对应**：Rust `Op… |
| ts-rust-boundary/optional-config-backward-compat.md#方案 | ts-rust-boundary | 方案 | ts-rust-boundary,option,backward-compat,unwrap_or,config-migration | active | **Rust 端** (`config.rs:11-25`)： ```rust pub struct MockConfi… |
| ts-rust-boundary/optional-config-backward-compat.md#用途 | ts-rust-boundary | 用途 | ts-rust-boundary,option,backward-compat,unwrap_or,config-migration | active | 配置迭代的通用方案，适用于： - 新增可选旋钮 - 旧版本平台配置升级 - 分阶段特性开关（旧特性先 disable，新… |
| ts-rust-boundary/optional-config-backward-compat.md#问题 | ts-rust-boundary | 问题 | ts-rust-boundary,option,backward-compat,unwrap_or,config-migration | active | 新旋钮常需跨 Rust↔TS 边界，并与旧配置字段共存以确保向后兼容。  例：`mock` 配置新增 `ttft_ms`… |

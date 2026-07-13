# 架构合理性审计 — aidog

范围: `src/` + `src-tauri/` 全结构,只读。基准: `CLAUDE.md`(项目结构 + 关键约束) + SKEIN Code Reuse Rules + 通用分层原则。
通用结论: Rust 后端经 C2-C9 战役已重构成 Cargo workspace(commands_system/proxy/cli_env/platform/config/ai_tools/tray + aidog_core),gateway 内按领域拆 15 子目录 + 16 扁平文件,整体方向健康。前端 feature-sliced(domains/)起步但未铺开。下列为仍存在的架构异味。

---

### F1: CLAUDE.md「项目结构」章节与实际 Cargo workspace 结构严重脱节
- 严重度: high
- 位置: `CLAUDE.md`「项目结构」「关键约束」全节 vs 实际 `src-tauri/src/lib.rs`(12 行)、`src-tauri/crates/*`
- 问题: CLAUDE.md 作为约束 / 真值源仍描述已不存在的旧结构:
  1. 写「`lib.rs # Tauri commands(73 个)`」——实际 `lib.rs` 仅 12 行模块声明,73 commands 已按领域下沉到 8 个 crate(`commands_system`/`commands_proxy`/`commands_cli_env`/`commands_platform`/`commands_config`/`commands_ai_tools`/`commands_tray` + `aidog_core`),见 `src-tauri/src/commands.rs:1-20` 注释自陈。
  2. 写「`gateway/adapter/... converter.rs / db.rs / proxy.rs / router.rs`」——实际路径为 `src-tauri/crates/aidog_core/src/gateway/<domain>/`,`gateway/` 不在 `src-tauri/src/` 下。
  3. 关键约束引「`commands/defaults.rs::get_defaults_json`」——实际在 `src-tauri/crates/commands_config/src/defaults.rs`;引「`gateway/peak_hours.rs`」——实际 `aidog_core/src/gateway/peak_hours.rs`;引「`src/services/api.ts`」——实际是 `src/services/api/` 目录(index.ts + 15 领域文件 + types/)。
  4. 写「11 个页面组件」——实际 `src/pages/` 含 18 个页面 + 5 个页面子目录(AppSettings/Groups/Logs/Mcp/PopoverConfigTab/platforms/Skills)。
  5. 写「`models.rs # 所有 Rust 数据模型(Protocol 枚举 62 变体)`」——实际 `aidog_core/src/gateway/models/` 目录,protocol 独立文件。
  - 违反: CLAUDE.md 规则「事实声明必须有引用」的基础——引用的 file:line 全部指向已不存在路径,任何按图索骥的 agent / 人都会扑空;「platform-presets.json 真值源」约束依赖的 Rust 入口路径(`commands/defaults.rs::get_defaults_json`)错位。
- 修复方向: 不动代码,同步 CLAUDE.md「项目结构」与「关键约束」内所有 Rust 路径到 crate 路径,commands 计数改为「按领域下沉 8 crate」叙述,页面对应更新。增量改文档,零行为变更。
- 影响面: CLAUDE.md 全文 Rust/前端路径引用(>20 处)、.wiki/ 跨引用。
- 跨维度: 无(纯文档真值源漂移,但污染所有维度的定位准确性,建议首先修复)。

### F2: feature-sliced 分层(domains/)仅覆盖部分 feature,边界规则未文档化
- 严重度: medium
- 位置: `src/domains/`(仅 `platforms`/`groups`/`shared`)vs `src/pages/{Logs,Mcp,Skills,PopoverConfigTab}/`、`src/components/{settings,platforms}/`
- 问题: `domains/` 是更清晰的 feature-sliced 层(逻辑 + UI 同居),但只落地 platforms/groups/shared 三个 feature。其余 feature(logs/mcp/skills/popover/settings)仍按技术类型散在 `pages/` + `components/`,且即便 platforms 自身也跨三层:`pages/platforms/`(12 文件)+ `components/platforms/`(4)+ `domains/platforms/`(10)——同一 feature 三处存放,无规则说明哪种产物落哪层。违反 LARGE_REPO「分层 CLAUDE.md」精神(本地约定缺)与 SKEIN Code Reuse「提取共享函数放语义正确目录」(目录语义本身未定)。
- 修复方向: 二选一并落档到 root CLAUDE.md「项目结构」:(a) 全面 feature-sliced,其余 feature 渐进迁入 `domains/<feature>/`;或 (b) 退回按技术层(pages/components/services),把 `domains/platforms`/`domains/groups` 内 UI 回流 components、逻辑回流 services。增量迁移,不强制一次性。
- 影响面: `src/domains/` 全部、`src/pages/{Logs,Mcp,Skills,platforms}/`、`src/components/platforms/`、`src/components/settings/`。
- 跨维度: 无。

### F3: gateway 模块组织门槛不统一(15 子目录 vs 16 扁平 .rs 并存)
- 严重度: medium
- 位置: `src-tauri/crates/aidog_core/src/gateway/mod.rs:1-31`
- 问题: gateway 下 15 个领域已升级为子目录(adapter/backup/db/estimate/hooks/middleware/mitm/models/notification/proxy/quota/router/skills/mcp/import_export,各自带 mod.rs + test_*),但同级别的 16 个仍是单文件(`peak_hours.rs`/`price_sync.rs`/`logo_sync.rs`/`defaults_sync.rs`/`client_types_sync.rs`/`i18n.rs`/`usage_color.rs`/`manual_budget.rs`/`time_models.rs`/`scheduling.rs`/`scripts.rs`/`codex.rs`/`coding_plan.rs`/`claude_integration.rs`/`log_util.rs`/`http_client.rs`)。升级为子目录的门槛(行数?测试?子模块?)未定义,新人无从判断新领域该扁平还是建目录。属「抽象层次错位」——同级概念两种组织形态。
- 修复方向: 定一条显式门槛(如「带 ≥1 test 文件或 ≥300 行 → 建子目录 + mod.rs」),把已带 test 的扁平文件(如 `test_codex.rs`/`test_i18n.rs` 已存在 → `codex`/`i18n` 应升级)按门槛统一。增量,每次触及再升级。
- 影响面: `aidog_core/src/gateway/mod.rs` + 16 扁平文件。
- 跨维度: 无。

### F4: 共享 Modal 基元缺失,22 处 createPortal 各自重实现
- 严重度: medium
- 位置: `src/components/shared/index.ts`(无 Modal 导出);9 个 `*Modal.tsx`(`UpdatePromptModal`/`UnsavedChangesModal`/`SmartPasteModal`/`ShareModal`/`WindowsEditModal`/`McpModals`/`GroupCreateModal`/`SkillModals`/`SegmentEditModal`)+ 13 处其他 createPortal
- 问题: CLAUDE.md 硬约束「modal 必须 createPortal(document.body)」只规定脱祖先,但未提供共享基元。结果是 22 处 createPortal 调用各自重写 backdrop/层级/escape/焦点管理,无 `components/shared/Modal.tsx`。违反 SKEIN Abstract Threshold「≥3 处相同逻辑必须 abstract」——modal 外壳是典型 ≥3 处重复。每次新 modal 复制粘贴,escape 行为 / 点击外部关闭 / aria 属性难统一。
- 修复方向: 抽 `components/shared/Modal.tsx`(props: open/onClose/children/size),内部固定 createPortal(document.body)+ backdrop + escape + focus trap,现有 modal 增量迁移。向后兼容(旧 modal 不强制立即改)。
- 影响面: 22 处 createPortal consumer,9 个 Modal 文件。
- 跨维度: 无。

### F5: 日期格式化绕过 formatters.ts(15 处内联 vs 5 处走统一)
- 严重度: medium
- 位置: `src/components/platforms/PlatformCard.tsx:277,313,385,402,429,893,895`、`src/components/platforms/SmartPasteModal.tsx:371`、`src/pages/Home.tsx:160,161`、`src/pages/TrayConfigTab.tsx` 等
- 问题: `src/utils/formatters.ts:76,90` 已导出 `formatDateTime`/`formatRelativeTime`,但全前端 15 处仍直接 `.toLocaleString()`(PlatformCard 占 7 处),仅 5 处用 formatters。违反 CLAUDE.md UI/i18n 硬约束「数值格式化统一走 utils/formatters.ts,禁页内重复定义」。日期格式无法随 locale 统一切换(8 语言),`toLocaleString()` 走浏览器默认 locale 而非应用 i18n locale。
- 修复方向: 把 15 处 `.toLocaleString()` 改调 `formatDateTime`(已有),如需 locale 感知再扩 formatters 入参。增量替换,TS 不阻。
- 影响面: PlatformCard/SmartPasteModal/Home/TrayConfigTab 等约 8 文件。
- 跨维度: 与 i18n 协同(locale 一致性)。

### F6: `services/api/types/part1-4.ts` 按尺寸机械切分而非按领域
- 严重度: low
- 位置: `src/services/api/types/part1.ts`(508)/`part2.ts`(440)/`part3.ts`(437)/`part4.ts`(93)、`types.ts`(8 行 barrel)
- 问题: 类型按「part 编号」切,无语义边界——新增类型不知落哪个 part,part4 仅 93 行说明切分不均。违反模块聚合原则(内聚按领域,非按文件大小)。
- 修复方向: 按领域重命名拆分(`platform.ts`/`group.ts`/`proxy.ts`/`settings.ts`/...),barrel 不变。增量: 每次改某领域类型时顺手迁移该域。
- 影响面: `src/services/api/types/` 4 文件 + barrel。
- 跨维度: 无。

### F7: 跨层重复逻辑(peak_hours / tray 渲染)手写双份,缺契约测试防漂移
- 严重度: low
- 位置: `aidog_core/src/gateway/peak_hours.rs` ↔ `src/utils/peakHours.ts`(99 行);`aidog_core/src/tray_render.rs` ↔ `src/pages/TrayConfigTab.tsx:92 computeItemText`(注释自陈「与后端 tray_segments 对齐」)
- 问题: CLAUDE.md 已文档化这两处 cross-layer 对称(peak_hours 段、TrayConfigTab 段),属已知设计权衡(前端同步 UI 不能等 IPC)。但双份手写逻辑仅靠注释「对齐」约束,无共享 fixtures / 契约测试,任一侧改判定规则另一侧静默漂移(典型: 跨天 end<start、start_at 生效、first-match 顺序)。属「跨层重复真值源」的轻量版。
- 修复方向: 抽一份共享 JSON fixtures(peak 窗口样例 + 期望命中结果),Rust 测试与 TS 测试(jest/vitest)同源消费,任一侧改判定先改 fixtures 触发对侧失败。tray 同理(段配置 + 期望渲染串)。
- 影响面: peak_hours.rs/peakHours.ts/test_peak_hours(若存在)、tray_render.rs/TrayConfigTab.tsx。
- 跨维度: 无。

### F8: `$${formatCost(x)}` 美元前缀重复,已有 formatCostUsd 未被采用
- 严重度: low
- 位置: `src/components/platforms/PlatformCard.tsx:667,675`、`src/pages/Groups/GroupListItem.tsx:233`、`src/pages/Groups/GroupListView.tsx:328`(≥3 处 `value={\`$${formatCost(...)}\`}`)
- 问题: `formatters.ts:41` 已有 `formatCostUsd`(自带 $),但 StatChip 的 cost 值用 `formatCost` + 手拼 `$`,≥3 处。违反 SKEIN Code Reuse「≥3 处相同逻辑必须 abstract」+ 「禁止绕过已有 utility 实现相同逻辑」。货币符号硬编码也妨碍 i18n(部分 locale 货币符号位置不同)。
- 修复方向: 改调 `formatCostUsd`,或让 StatChip 接 `currency` prop 内化。一行级替换。
- 影响面: PlatformCard + Groups 两处 ListView/ListItem。
- 跨维度: 无。

### F9: `domains/` 混 UI 组件与纯逻辑,与 SKEIN 目录职责规则冲突
- 严重度: low
- 位置: `src/domains/platforms/{MockConfigEditor.tsx,ProtocolLogo.tsx,SearchableProtocolSelect.tsx}` + `.ts` 逻辑(autoCategorize/defaults/health);`src/domains/groups/{GroupIcon.tsx,GroupTestPanel.tsx,PlatformPicker.tsx}` + `.ts` 逻辑
- 问题: SKEIN Code Reuse 明定「UI 相关 → src/components/,数据/业务 → src/services/」,但 `domains/` 把 UI(.tsx)与逻辑(.ts)同居。规则未更新承认 domains/ 层 → 规则与现实分叉,执行时无据。属「抽象层次规则与代码现实错位」。
- 修复方向: 二选一:(a) 正名 domains/ 为 feature 层并更新 SKEIN 规则的目录表增「feature 切片 → src/domains/<feature>/」;(b) 严格按旧规则,UI 回 components/、逻辑回 services/。建议 (a)(feature-sliced 更适合本仓规模)。
- 影响面: 与 F2 同处置。
- 跨维度: 无。

### F10: `src/popover.tsx` 根级页面态文件游离于 pages/ 之外
- 严重度: low
- 位置: `src/popover.tsx`(197 行,根级)、`src/pages/PopoverConfigTab.tsx`(15 行,薄壳)、`src/components/PopoverCards.tsx`(638 行)
- 问题: popover 功能拆三文件横跨三层(根 src/ / pages/ / components/),其中 `src/popover.tsx` 是独立窗口入口(Tauri 多窗口)放在仓库根而非 `src/pages/` 或 `src/entries/`,与前端约定(页面在 pages/)不符。新人难定位 popover 入口。
- 修复方向: 建 `src/entries/`(或 `src/windows/`)收多窗口入口(popover / tray / main),与路由页(pages/)区分。增量: 移动 + import 路径更新。
- 影响面: `src/popover.tsx` + tauri.conf.json 的窗口入口配置 + main.tsx 入口注册。
- 跨维度: 无。

---

## 摘要(按严重度)

| ID | 严重度 | 标题 | 位置 |
|---|---|---|---|
| F1 | high | CLAUDE.md 项目结构 / Rust 路径全面脱节(lib.rs 73commands 实为 8 crate workspace) | CLAUDE.md 全节 |
| F2 | medium | feature-sliced(domains/)仅覆盖 3 feature,边界未文档化 | src/domains vs pages/components |
| F3 | medium | gateway 15 子目录 vs 16 扁平文件,升级门槛缺 | aidog_core/src/gateway/mod.rs |
| F4 | medium | 共享 Modal 基元缺失,22 createPortal 各自重实现 | components/shared 无 Modal |
| F5 | medium | 日期格式化 15 处绕过 formatters.ts(违反硬约束) | PlatformCard/Home/TrayConfigTab |
| F6 | low | types/part1-4 按尺寸切分非按领域 | services/api/types/ |
| F7 | low | peak_hours / tray 渲染跨层双份,缺契约测试 | peak_hours↔peakHours.ts / tray_render↔TrayConfigTab |
| F8 | low | `$${formatCost}` 重复,formatCostUsd 未采用 | PlatformCard/Groups |
| F9 | low | domains/ 混 UI 与逻辑,与 SKEIN 目录职责规则冲突 | domains/platforms,domains/groups |
| F10 | low | src/popover.tsx 根级窗口入口游离 pages 外 | src/popover.tsx |

## 未覆盖
- 前端 react-router 缺失下的导航契约(App.tsx 侧栏 + AppSettings tab 本地 state)仅粗看,未深审 navGuard 注册表边界冲突 —— 归 code-quality。
- import_export / ccswitch 子系统内部职责切分(apply/collect/container)未深读,目录结构本身健康。
- Rust crate 间依赖方向(commands_* → aidog_core 是否单向)未画依赖图,目测无环但未验证 —— 可补 audit-boundary。

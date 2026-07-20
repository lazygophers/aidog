# 代码质量审计 — aidog

只读审计「代码质量 / 重复 / 复杂度」维度。范围：`src/**/*.{ts,tsx}` + `src-tauri/crates/**/*.rs`（仓库正处于 workspace 拆分重构期，Rust 代码已从 `src-tauri/src/gateway/` 迁入 `src-tauri/crates/{aidog_core,commands_*}/`，CLAUDE.md 描述的旧路径已失效但非本审计范围）。

## 摘要表（按严重度排序）

| ID | 严重度 | 标题 | 位置 |
|---|---|---|---|
| F1 | high | 179 处 `#[allow(unused_imports)]` 掩盖死 import | 7 个 crate（commands_system 36 / platform 36 / aidog_core 37 / ai_tools 27 / proxy 24 / tray 13 / config 7） |
| F2 | high | `console.log` debug 残留生产代码 | src/pages/AppSettings/LogSettingsSection.tsx:33,36 |
| F3 | medium | `updateField: (field: string, value: any)` 重复 10+ 处且无类型 | src/components/settings/editors/* |
| F4 | medium | `catch (e: any)` 32 处应走 `unknown` + narrowing | src/ 全局 |
| F5 | medium | PlatformCard 914 行 god component / ~20 props | src/components/platforms/PlatformCard.tsx:77 |
| F6 | medium | `pad` 工具重复 3 处（同文件内 2 次），formatters.ts 缺公共版 | formSections.tsx:26,416 + ScheduledBackupSection.tsx:25 |
| F7 | medium | `clamp` 重复 3 处签名各异 | popover.tsx:76 / PlatformCard.tsx:790 / shared/usageColor.ts:56 |
| F8 | medium | ANSI_RE 正则在 catalog.rs 两函数各 OnceLock 定义一次 | catalog.rs:90-91,232-233 |
| F9 | medium | `select_candidates_ctx` 232 行 / 19 分支高圈复杂度 | router/candidates.rs:46 |
| F10 | medium | `model_test` 272 行线性长函数 | commands_ai_tools/src/model_test.rs:18 |
| F11 | low | `relativeTime` 与 formatters.ts `formatRelativeTime` 语义重叠且硬编码英文 | PlatformCard.tsx:774 |
| F12 | low | 26 处 eslint-disable exhaustive-deps 部分或掩盖真实依赖 bug | src/ 全局 |
| F13 | low | Mutex `.lock().unwrap()` 5 处非 test，poison 即 panic | proxy/log.rs / mitm/cert_signer.rs |
| F14 | low | commands 文件头部 7 行 import 模板逐字重复 13 处 | src-tauri/crates/commands_*/src/*.rs |

---

## F1: 179 处 `#[allow(unused_imports)]` 掩盖死 import
- 严重度: high
- 位置: `src-tauri/crates/` 全 7 个 commands_*/aidog_core crate；按 crate 分布：commands_system 36 / commands_platform 36 / aidog_core 37 / commands_ai_tools 27 / commands_proxy 24 / commands_tray 13 / commands_config 7（`grep -r 'allow(unused_imports)' --include='*.rs' | wc -l` = 179）。
- 问题: 每个 command 文件头部抄同一份 7 行 import 模板，未用项用 `#[allow(unused_imports)]` 压住编译警告。例 `commands_system/src/app_log.rs:2-12`：
  ```rust
  use aidog_core::gateway::{self, db::{self, Db}};
  #[allow(unused_imports)] use aidog_core::logging;
  #[allow(unused_imports)] use gateway::models::*;
  #[allow(unused_imports)] use tauri::State;
  #[allow(unused_imports)] use serde_json::Value;
  #[allow(unused_imports)] use std::sync::Arc;
  #[allow(unused_imports)] use tauri::Manager;
  ```
  该文件实际只用 `logging` 与 `db`/`Db`，其余 5 行 import 全是死代码被 allow 蒙住。workspace 拆分重构（见根 Cargo.toml 注释 C1/C2 阶段）后未清理。
- 修复方向: 逐文件删未用 import 与对应 `#[allow(unused_imports)]`；可临时去掉 allow 让编译器一次性列全未用项再批量清。向后兼容（不影响公开 API）。
- 风险: 低；纯删死代码，编译器兜底。需全量 `cargo build` 确认无隐式宏依赖被误删。
- 跨维度: 无

## F2: `console.log` debug 残留生产代码
- 严重度: high
- 位置: `src/pages/AppSettings/LogSettingsSection.tsx:33,36`
- 问题: `handleCleanupExpired` 内裸 `console.log("[LogSettings] cleanupExpired start")` / `console.log("[LogSettings] cleanupExpired done")` 未走任何 log 开关，每次用户点「清理过期日志」直接打到桌面 WebView 控制台。同期 `catch (e)` 用 `console.error` 是合理的，但正常路径的 debug log 属残留。
- 修复方向: 删两行 `console.log`（保留 `console.error`）。如需 debug 可走后端 `tracing` 或前端 dev-only `if (import.meta.env.DEV)` 守卫。
- 风险: 无。
- 跨维度: 无

## F3: `updateField: (field: string, value: any)` 重复 10+ 处且无类型
- 严重度: medium
- 位置: `src/components/settings/editors/` 多文件：SandboxSection.tsx:130,395,408 / StatusLineSection/StatusLinePanel.tsx:25 / StatusLineSection/useStatusLinePanel.ts:26 / hooks-types.tsx:120 / StatusLineSection.tsx:23 / HooksSection.tsx:24 / PluginsSection.tsx:189,356（`grep -rn 'updateField: (field: string, value: any)'`）。
- 问题: 编辑器子组件统一用一个 `value: any` 的 setter 接收任意字段任意值，丢光类型安全；调用方写错字段名 / 传错类型 TS 不报。同一签名在 ≥10 处重复，是 settings 编辑器体系的事实接口但未抽公共类型。
- 修复方向: 在 `editors/_shared.tsx` 定义泛型 `UpdateField<T> = (field: keyof T, value: T[keyof T]) => void`（或保持 string field 但 value 收窄为 `unknown` 后在 caller narrowing），子组件改用该类型。向后兼容（仅类型层，不改运行时）。
- 风险: 中；改类型会波及所有 settings 子组件，需全量 `yarn build` 验证。泛型化可能触动现有 `field: string` 的动态拼字段调用点。
- 跨维度: 无

## F4: `catch (e: any)` 32 处应走 `unknown` + narrowing
- 严重度: medium
- 位置: `src/` 全局 32 处（`grep -rn 'catch (e: any)'` = 32），典型如 CodingToolsSettings.tsx:103,145,175 / useStatusLinePanel.ts:104,136。
- 问题: TS 4.4+ `useUnknownInCatch` 默认下 `catch (e)` 已是 `unknown`，显式标 `: any` 是退化——后续 `String(e)` / `e.message` 访问绕过类型检查，错误处理路径无类型保护。
- 修复方向: 去掉 `: any`（让 e 回到 `unknown`），调用处统一走 `String(e)` 或封装 `errorMessage(e: unknown): string` helper（已有 `flashMessage(String(e))` 模式，直接删 `: any` 即可）。向后兼容。
- 风险: 低；纯类型收紧，`String(unknown)` 合法。少数直接读 `e.message` / `e.code` 的点需补 narrowing。
- 跨维度: 无

## F5: PlatformCard 914 行 god component / ~20 props
- 严重度: medium
- 位置: `src/components/platforms/PlatformCard.tsx:77-113`（props 解构 20 项），全文件 914 行。
- 问题: 单个 `memo` 组件承担：展示（CompactCard/StatChip/BalanceBar）、配额计算（computeQuotaDisplay）、用量着色（successRateLevel/costLevel）、拖拽（isDragging/dragActive）、展开态（expanded）、模型测试（testing/manual/lastTest）、favicon 回退、分组优先级编辑（LevelPriorityControl）。props 列表含 platform/quotaRaw/quotaPreferReal/refreshing/quotaPending/usagePending/usage/expanded/manualResult/testing/faviconFailed/actions/platformMembership/draggable/lastTest/levelPriority/onLevelPriorityChange 等 ~20 项，是典型 god component + 长参数列表坏味道。
- 修复方向: 按职责切子组件（已部分抽出 `LevelPriorityControl` / `LastTestBadge` / `relativeTime`）：测试结果区（LastTestBadge 已抽，可继续把测试触发逻辑入 hooks）、配额展示区、拖拽壳层分别独立；props 聚合成 `PlatformCardData` + `PlatformCardActions` 两个对象降参数数。向后兼容（纯前端组件内部拆分，不改对父契约——但 props 聚合成对象会改契约，可先内部拆子组件，props 聚合留后续）。
- 风险: 中；memo 浅比较边界变化可能引入重渲回归，需手动验证列表滚动 / 展开交互。
- 跨维度: 与 audit-perf 协同（god component 通常也是重渲源）

## F6: `pad` 工具重复 3 处（同文件内 2 次）
- 严重度: medium
- 位置: `src/pages/platforms/formSections.tsx:26`、`src/pages/platforms/formSections.tsx:416`（同一文件两处）、`src/components/settings/ImportExport/ScheduledBackupSection.tsx:25`。
- 问题: 三处各自 `const pad = (n: number) => String(n).padStart(2, "0")`，逻辑完全相同。formatters.ts（src/utils/formatters.ts）已是项目统一格式化工具收敛点却缺 `pad`。同文件内重复两次尤其明显（toDatetimeLocal 与 formatWindowPreview 各定义一份）。
- 修复方向: 在 formatters.ts 导出 `pad2(n: number): string`，三处改 import。向后兼容。
- 风险: 无。
- 跨维度: 无

## F7: `clamp` 重复 3 处签名各异
- 严重度: medium
- 位置: `src/popover.tsx:76` `clamp(v, lo, hi)`、`src/components/shared/usageColor.ts:56` `clamp(v, lo, hi)`、`src/components/platforms/PlatformCard.tsx:790` `clamp(v)`（硬编码 1–10）。
- 问题: popover 与 usageColor 两处 `clamp(v, lo, hi) => Math.min(hi, Math.max(lo, v))` 实现等价（仅参数顺序写法微异），PlatformCard 的 `clamp(v)` 是专用 1–10 版。两份通用 clamp 抄两份。
- 修复方向: formatters.ts 导出通用 `clamp(v, lo, hi)`，popover + usageColor 改 import；PlatformCard 的 1–10 专用版可内联或保留（语义不同，算合理 inline）。向后兼容。
- 风险: 无。
- 跨维度: 无

## F8: ANSI_RE 正则在 catalog.rs 两函数各 OnceLock 定义一次
- 严重度: medium
- 位置: `src-tauri/crates/aidog_core/src/gateway/skills/catalog.rs:90-91`（`parse_add_list_output`）与 `:232-233`（`parse_find_output`）。
- 问题: 两处用各自 `static ANSI_RE: OnceLock<Regex>` + 相同正则 `r"\x1b\[[0-9;?]*[A-Za-z]"` 做 ANSI 转义剥离，OnceLock + 正则字面量逐字重复。
- 修复方向: 抽 crate 级 `static ANSI_RE: OnceLock<Regex>`（或 `lazy` 常量），两函数共用 `.get_or_init(...)`。向后兼容（内部重构）。
- 风险: 无。
- 跨维度: 无

## F9: `select_candidates_ctx` 232 行 / 19 分支高圈复杂度
- 严重度: medium
- 位置: `src-tauri/crates/aidog_core/src/gateway/router/candidates.rs:46-278`。
- 问题: 单函数 232 行，内含 19 个 if/match/for 分支，覆盖：模型映射查找 → 单平台组 bypass → 高峰禁用判定 → 多平台筛选（status / 熔断 / 过期 / 高峰多维过滤）→ 调度策略排序。是路由核心，但分支密集，新维度（如 disable_during_peak）只能继续往里堆，维护与测试都难。
- 修复方向: 按已有维度抽 filter 闭包链（候选过滤已天然是 pipeline）：`collect_candidates` → 依次过 `filter_disabled` / `filter_peak_disabled` / `filter_breaker` / `filter_expired`，每步独立可测；单平台 bypass 作为前置 short-circuit。不改对外 `select_candidates_ctx` 签名与返回 `CandidateSet`。向后兼容。
- 风险: 中；路由是核心热路径，拆分需保 `cargo test`（router/test_candidates.rs、test_ordering.rs）全绿作回归网。
- 跨维度: 与 audit-architecture 协同（模块边界）

## F10: `model_test` 272 行线性长函数
- 严重度: medium
- 位置: `src-tauri/crates/commands_ai_tools/src/model_test.rs:18-290`。
- 问题: Tauri command 单函数 272 行，线性串联：取 platform → 解析 model/prompt（自定义 vs 随机挑战）→ 构 ChatRequest → 调 adapter → 解析响应 → 内容校验（expected 子串）→ 装配 ModelTestResult。10 个分支，虽不深嵌套但单函数职责过多，随机挑战生成、请求构造、结果校验三段逻辑耦合在一起。
- 修复方向: 抽 `build_test_request(platform, req) -> ChatRequest`、`validate_response(content, expected) -> bool`、`random_test_challenge()` 已独立（可保持），command 函数只做编排。不改 `#[tauri::command]` 签名。向后兼容。
- 风险: 低；纯内部拆分，有 model_test 相关 test 兜底。
- 跨维度: 无

## F11: `relativeTime` 与 formatters.ts `formatRelativeTime` 语义重叠且硬编码英文
- 严重度: low
- 位置: `src/components/platforms/PlatformCard.tsx:774-784`（`relativeTime`，返回 `"3m"`/`"3h"`/`"3d"`/`""`），对照 `src/utils/formatters.ts:formatRelativeTime`（返回 `"3 分钟前"` 等）。
- 问题: 两函数都做「毫秒差 → 相对时间」，PlatformCard 版输出硬编码英文缩写且 <60s 返回空串，与 formatters 版（中文、<60s 返「刚刚」）分叉。PlatformCard 版用于 LastTestBadge 副信息，英文硬编码绕过 i18n。
- 修复方向: 两选一——① formatters.ts 增 `formatRelativeTimeCompact`（含 i18n + 紧凑模式参数），PlatformCard 改调；② 若紧凑英文徽标是有意设计（空间受限），加注释说明并保留。向后兼容。
- 风险: 低。
- 跨维度: 与 i18n 维度交叉（英文硬编码），标注不展开

## F12: 26 处 eslint-disable exhaustive-deps 部分或掩盖真实依赖 bug
- 严重度: low
- 位置: `src/` 全局 26 处，密集区：PlatformCard.tsx（5 处：112,124,139,196,204）、SmartPasteModal.tsx（2 处）、useSkillsData.ts（3 处）、AppContext.tsx:220 等。
- 问题: 多数是「有意只依赖某一项、省略稳定回调」的合理 suppress，但 26 处里有部分把整个依赖数组写成 `[p.platform_type]` 而闭包内读了 `setColor`/`p` 等更多值——若被省略的依赖后续变化（如 platform 对象引用变更）会闭包捕获旧值。逐个核查成本高，但其中掩盖真实 stale-closure 的概率非零。
- 修复方向: 逐处核对：合理的保留并加 `// deps: 仅依赖 X，Y 为稳定引用` 注释；可疑的补全依赖或改用 `useCallback` 稳定化。向后兼容。
- 风险: 低；改错会触发额外 effect 执行，需手测对应交互。
- 跨维度: 与 audit-perf 协同（effect 依赖与重渲强相关）

## F13: Mutex `.lock().unwrap()` 5 处非 test，poison 即 panic
- 严重度: low
- 位置: `src-tauri/crates/aidog_core/src/gateway/proxy/log.rs:130,138,146,174`、`src-tauri/crates/aidog_core/src/gateway/mitm/cert_signer.rs:85,92`。
- 问题: `.lock().unwrap()` 是 Rust 常见写法，但 mutex poison（持锁线程 panic）时会二次 panic 致进程退出。proxy 热路径上 log_snapshots 与 cert_signer cache 的锁若遇 poison 会拖垮整个代理。
- 修复方向: 评估后——若接受「poison = 不可恢复」语义则保留（idiomatic）；若要更韧，改 `.lock().unwrap_or_else(|e| e.into_inner())` 显式忽略 poison（仅在「锁内数据损坏可接受」时）。向后兼容。
- 风险: 低；改动面小，但 poison 处理策略需与 team 对齐。
- 跨维度: 无

## F14: commands 文件头部 7 行 import 模板逐字重复 13 处
- 严重度: low
- 位置: `src-tauri/crates/commands_*/src/*.rs`（`grep -rl 'use aidog_core::gateway::{self, db::{self, Db}};'` = 13 文件）。
- 问题: 每个 command 文件开头逐字抄同一份 import 清单（gateway / logging / models::\* / tauri::State / serde_json::Value / std::sync::Arc / tauri::Manager），未用项靠 F1 的 `#[allow(unused_imports)]` 压住。模板化抄写导致每新增 command 都拷一份，死代码随模板扩散。
- 修复方向: 与 F1 合并治理——清完死 import 后，剩余真实 import 各文件按需写（不强制统一模板）；或抽一个 prelude（但 Rust prelude 对 workspace 子 crate 收益有限，YAGNI，倾向「按需 import + 删 allow」）。向后兼容。
- 风险: 无。
- 跨维度: 与 F1 同源

---

## 未覆盖

以下文件因体量 / 领域特性未深入逐行，仅扫了头部结构，列出供后续补扫：
- `src-tauri/crates/aidog_core/src/gateway/db/schema_late.rs`（1121 行）：DB migration 代码，长是本性，未审内部重复。
- `src-tauri/crates/aidog_core/src/gateway/mitm/ca.rs`（1223 行）：MITM CA 证书签发，领域密集，未审。
- `src-tauri/crates/aidog_core/src/logging.rs`（909 行）：日志子系统，未深入。
- `src/pages/platforms/formSections.tsx`（1009 行）：已扫结构（18 个独立 FormSection 子组件，拆分合理），但各 section 内部重复（如 PeakHoursSection 的 add/remove/toggleDay/addModel/update 五个 handler 模式）未逐一定级。
- `src/components/settings/statusline-runtime.ts`（762 行）+ `statusline-segments.ts`（785 行）：statusline 生成器，前者标 `AUTO-GENERATED ... DO NOT EDIT` + `/* eslint-disable */`（生成代码，合理豁免，不计 finding），后者未深入。
- `src-tauri/crates/aidog_core/src/gateway/proxy/forward.rs`（973 行）/ `connect.rs`（756 行）：代理转发核心，仅抽查 unwrap 点未全文审重复。
- 前端各 `test.*.tsx` 测试文件：不在代码质量审计范围。

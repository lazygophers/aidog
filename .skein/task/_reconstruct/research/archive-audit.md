# 归档规则逐条比对（reconstruct --deep=max）

归档源：`.skein/spec/.archive/1785510844/`（134 条）
排除：21 条 `protected: true`（已留在 `.skein/spec/` 库内，不在比对范围）
比对面：**113 条**

判定四档：仍成立 / 需改写 / 已过期 / 本就可疑

---

## 0. 全局漂移事实（多条规则受其影响，先列一次）

| 事实 | 证据 |
|---|---|
| `commands_*` 多 crate 已合并回 `aidog_core`，workspace 现仅 `aidog_core` + `aidog_test_util` | `ls src-tauri/crates/` → 只有两个；`crates/aidog_core/src/{platform_cmd,proxy_cmd,ai_tools_cmd,cli_proxy_cmd,system_cmd}/` |
| 全部 gateway 代码路径由 `src-tauri/src/gateway/**` 迁到 `src-tauri/crates/aidog_core/src/gateway/**` | `ls src-tauri/crates/aidog_core/src/gateway/` |
| `cpa_import` 模块已改名 `cli_proxy_parser` | `crates/aidog_core/src/gateway/cli_proxy_parser/parser.rs:186,387,400` |
| 前端 `src/services/api.ts` 单文件 → `src/services/api/` 目录 | `src/services/api/` 存在，`src/services/api.ts` 不存在 |
| 前端域层 `src/domains/{platforms,groups,shared}/` 已成型，`src/pages/platforms/` 为表单层 | `ls src/domains/`、`ls src/pages/platforms/` |
| `sonner.tsx` 组件已删（`next-themes` / `sonner` 仍留在 package.json 为未用依赖） | `src/components/ui/sonner.tsx` 不存在；`grep -rn next-themes src/` 零命中；`package.json:63,71` |

---

## 1. core/（4 条）

| archive 内路径 | 一句话内容 | 判定 | 证据 |
|---|---|---|---|
| `core/arch/stream-buf-unified-cap.md` | 流缓冲上界必须单一常量定义，多路径引用 | **仍成立** | `gateway/proxy/stream.rs:16` 单处 `const SSE_LINE_BUF_MAX_BYTES = 1MB`；`:97`/`:153`/`:272` 三处引用同一常量 |
| `core/arch/stream-buffer-cap-single-source.md` | 同上，仅剩标题+一行，无正文 | **本就可疑（重复组）** | 全文仅 frontmatter + `## 流缓冲上界单一真值源` 一行，无 MUST/验收/案例。与上一条同题，是空壳 |
| `core/db/sqlite-read-cache-config.md` | 只读连接 `cache_size=-64`，写连接不动默认 | **仍成立** | `gateway/db/mod.rs:372` `const READ_CACHE_DEFAULT_KB: i64 = 64`；`:374` env 旋钮 `AIDOG_SQLITE_READ_CACHE_KB` 仍在 |
| `core/domain/rule-67.md` | estimate 链余额扣减+手动预算必须同乘 peak 倍率 | **需改写** | 内核对：`gateway/estimate/db_ops.rs:219` 与 `:237` 两处均 `* peak_mult`（`:200` 取 `resolve_multiplier`）。但行号已漂（规则写 214/233，实际 219/237），且实现不是规则里写的 `maybe_peak_multiplier` 而是 `peak_hours::resolve_multiplier` |

---

## 2. recall/arch/（17 条）

| archive 内路径 | 一句话内容 | 判定 | 证据 |
|---|---|---|---|
| `auto-fix-downgrade-33.md` | agent-as-LLM 平台走 handler 分支拦截，不塞 wire 层 | **仍成立** | `gateway/proxy/handler.rs:412` `matches!(first.platform.platform_type, Protocol::Mock)` → `:418 return handle_mock(...)`，正是所述范式 |
| `auto-fix-downgrade-34.md` | 拆库审计必须同查 wrapper / `.write_conn`/`.read_conn` / 裸 SQL 三形式 | **仍成立** | `call_platform_traced` 仍在（`cli_proxy_cmd/batch.rs:31`）、`call_read_traced` 仍在（`platform_cmd/batch.rs:187`）；多库（log.db / platform.db）结构未变 |
| `auto-fix-downgrade-35.md` | 禁用「设计为空」的字段作 dedup key | **仍成立** | 语言无关通用工程规则，无需锚点；表述具体、有失败模式 |
| `auto-fix-downgrade-38.md` | 删 serde 落库 enum 变体前先 migration DELETE 旧值 | **仍成立** | `gateway/models/protocol.rs` Protocol 仍是 serde rename 落库 enum（`:236` `("glm_coding", Protocol::GlmCoding)`），机制成立 |
| `coding-plan-utilization-calib-fix-25.md` | 校准链 base_url 真值源 = endpoint 级，禁传 `platform.base_url` | **需改写** | 内核对（`ai_tools_cmd/model_test.rs:79` 仍从 `ctx.platform.endpoints` 取 `target_base_url`，`:90` 空则报错）。但规则里 `forward.rs` 的定位需改为「endpoint 解析处」；`est_coding_plan` 仍在（`platform_cmd/quota.rs:57`） |
| `cross-db-subquery-handle-selection.md` | 跨库补查闭包 handle 按被补查表的库归属选 | **仍成立** | 多库并存 + `call_platform_traced`/`call_read_traced` 分离 handle 仍是现状 |
| `non-typical-sql-audit-pattern.md` | 拆库审计禁只 grep helper 名，必须同查裸 SQL | **仍成立** | 与 `auto-fix-downgrade-34` 同族，仍适用；建议与之合并 |
| `parser-multi-path-format-symmetry.md` | parser 多入口识别同一格式必须对称 | **需改写** | 内核成立且锚点仍在，但模块已改名：规则写 `gateway/cpa_import/parser.rs:532`，现为 `gateway/cli_proxy_parser/parser.rs`（`:186 parse_single_file`、`:400 scan_auth_dir`） |
| `rule-49.md` | Tauri 托盘浮窗窗口复用（create-once + hide/show） | **仍成立** | `src/app_setup.rs:552 ns_window.setHidesOnDeactivate(true)`；`:347 is_visible()` 判 toggle；`:373 emit("popover-shown")`；`src/popover.tsx:156` 前端监听刷新 |
| `rule-56.md` | Gemini SSE 需拼 `?alt=sse` | **需改写** | 行为仍在但行号漂移：规则写 `forward.rs:203-211`，实际 `gateway/proxy/forward.rs:309-311` |
| `rule-57.md` | 协议名统一走 `Protocol::wire_str()` | **仍成立** | `gateway/models/protocol.rs:173 pub fn wire_str(&self)`，行号都没变 |
| `rule-58.md` | adapter 死代码判定权威 = `is_valid_wire_protocol` 白名单 | **仍成立** | `gateway/proxy/forward.rs:85` 闭包定义，`:88` gate，行号未漂 |
| `rule-59.md` | 抽组件必须 grep 确认所有调用点已切换 | **仍成立** | 通用重构纪律，无锚点依赖 |
| `rule-60.md` | invoke 名真值源 = `startup.rs` 的 `generate_handler!` 集合 | **需改写** | 机制成立（`src-tauri/src/startup.rs:41` `tauri::generate_handler![`），行号从 `:41` 未变；但「跨 crate 搬迁」的语境已消失（commands_* 已合并），改写时应把触发场景改为「command 增删改名」 |
| `rule-62.md` | 搬迁类重构用 `comm -23` 核对 i18n key 集合 | **需改写** | 方法本身有效，但「跨 crate 搬迁」语境已不存在；改写为「组件/页面改名或迁目录时的 i18n key 差集核对」，配合 `scripts/check-i18n.mjs` |
| `rule-64.md` | `tauri_command!` 宏不支持 `mut` 形参 | **仍成立** | `crates/aidog_core/src/command_macro.rs:12` 宏模式 `$($arg:ident : $ty:ty),*`，确实不匹配 `mut x: T` |
| `shadcn-infra-32.md` | 删主题/功能导致的 locale 死键由删除方同源清理 | **仍成立** | 流程约定，8 locale 结构未变（`ls src/locales/` 8 个 json） |
| `trellis-03.md` | Workspace crate 边界契约：commands_* 间禁互依赖 | **已过期** | `ls src-tauri/crates/` 只剩 `aidog_core` / `aidog_test_util`，全部 `commands_*` crate 已不存在；规则的 5 条边界规则、验收 grep 全部落空 |
| `trellis-04.md` | 新增 Protocol 变体前先 grep 同构变体命中点 | **需改写** | 内核仍成立（`protocol.rs:31 GlmCoding` + `:236` serde 映射）。漂移：规则里「必改点 2」的 `coding_plan.rs::default_is_coding_plan` 已不存在（grep 零命中），该条应删 |
| `trellis-05.md` | 前端大枚举常量派生自后端 JSON，module 级 `docPromise` 单次 RPC | **仍成立** | `src/domains/platforms/defaults.ts:85 let docPromise`、`:88-105` 单次缓存；`:126` 三分支 default/coding_plan/peak 与 CLAUDE.md 一致。小常量例外也仍在：`src/domains/platforms/constants.ts:11 ENDPOINT_PROTOCOLS`、`:30 PROTOCOL_LABELS` |

> 注：`recall/arch/` 非 protected 共 20 条（上表 20 行）。

---

## 3. recall/build/（10 条）

| archive 内路径 | 一句话内容 | 判定 | 证据 |
|---|---|---|---|
| `rule-05.md` | 新增 wire protocol 必须同步白名单 + converter 两侧 match | **仍成立** | `gateway/proxy/forward.rs:85` 白名单；`gateway/adapter/converter/request.rs:12 convert_request`；`gateway/adapter/converter/response.rs:10 parse_sse`。三处齐全（路径需补 `adapter/` 前缀） |
| `rule-06.md` | converter 5×5 与 endpoint 选择解耦 | **仍成立** | `gateway/proxy/endpoint.rs:95 select_endpoint_for_protocol` 与 converter 双向转分属两模块，无互相依赖 |
| `rule-07.md` | `is_valid_wire_protocol` gate 是 fail-fast 非修复点 | **仍成立** | `forward.rs:85-88` gate 仍在原位，根因仍在 `endpoint.rs:95` |
| `rule-61.md` | clippy 缓存命中不重报 warning，改前先 touch | **仍成立** | cargo 行为，与代码结构无关 |
| `rule-63.md` | `env!()` 编译期常量随跨 crate 搬迁失效 | **需改写** | 机制真（cargo 行为），但案例全部指向已不存在的 `commands_tray/build.rs`；现只有 `src-tauri/build.rs` + `crates/aidog_core/build.rs`。改写为「新建 crate 时 build.rs 的 `cargo:rustc-env` 需各自定义」，删 commands_* 案例 |
| `shadcn-infra-02.md` | Tailwind v4 禁 v3 三行 `@tailwind` 导入 | **仍成立** | `src/styles/globals.css:4-6` 用 `@layer` 声明 + `@import "tailwindcss/..." layer(...)`，无任何 `@tailwind` 指令；`package.json:89 tailwindcss ^4.3.3` |
| `shadcn-infra-28.md` | `shadcn add` 漏装 cva，add 后须验证 | **仍成立** | `package.json:58 class-variance-authority ^0.7.1` 已在（当时补装的结果）；规则是流程纪律，仍适用 |
| `shadcn-infra-29.md` | vite `@` alias 需手动配置 | **仍成立** | `vite.config.ts:15-16` 有 `// ponytail: @ alias 供 shadcn 组件解析` + `alias:` 块 |
| `shadcn/shadcn-primitives-39.md` | shadcn add 后必 grep package.json 验证依赖 | **需改写（与 shadcn-infra-28 重复组）** | 与 `shadcn-infra-28` 同题同结论。`-39` 多列了具体缺失包清单（cva/lucide-react/vaul/sonner/cmdk），`-28` 多了 `yarn why` 验收命令。建议合并保留一条 |
| `tauri-build-bundle.md` | `--no-bundle` 不产 `.app`，要 `.app` 用 `--bundles app` | **仍成立** | Tauri CLI 行为，与本仓代码无关；`src-tauri/tauri.conf.json` 仍是 Tauri 2 工程 |
| `trellis-02.md` | 单 crate→workspace 重构必须先过空骨架 PoC 门禁 | **需改写** | workspace 已建成且已反向收敛（commands_* 合并回 aidog_core），「重构过程门禁」这一具体场景已完成、不复现；但 `[workspace.dependencies]` 版本对齐、binary 同名延后两条仍是有效的 workspace 卫生规则。建议降为一条精简的 workspace 卫生规则，删 PoC 迁移流程 |

---

## 4. recall/cross-layer/（2 条）

| archive 内路径 | 一句话内容 | 判定 | 证据 |
|---|---|---|---|
| `trellis-20.md` | Tauri↔React 边界契约：新增 command 必配前端 invoke 包装 + snake_case 字段 | **需改写** | 内核仍是本仓最高频 bug 源。漂移：规则通篇写 `src/services/api.ts` 单文件，现为 `src/services/api/` 目录（`index.ts` + `types/` + 各模块 `.ts`）；路径全部要重写。`src-tauri/src/startup.rs:29` 等 app 层锚点仍在 |
| `ts-rust-symmetry.md` | 单启用平台判定 Rust↔TS 各一份，改一处必改另一处 | **仍成立** | `crates/aidog_core/src/gateway/router/mod.rs:98 pub(crate) fn sole_platform`（行号未漂）；`src/domains/groups/GroupIcon.tsx` 存在。两端「不考虑 expires_at/disable_during_peak」的口径与 CLAUDE.md 一致 |

---

## 5. recall/db/（7 条，protected 1 条已排除）

| archive 内路径 | 一句话内容 | 判定 | 证据 |
|---|---|---|---|
| `crash-safe-db-split-migration.md` | 拆库迁移四阶段 crash-safe（read-without-drop 先行） | **仍成立** | 多库结构仍在（log.db / platform.db，见 `db/` 目录 + CLAUDE.md）；规则是不可逆数据操作的安全模式，价值高 |
| `filter-semantics.md` | 「默认排斥某类请求」的过滤须确认为产品意图 | **需改写** | 结论仍成立（`db/proxy_log.rs:403,409` 注释明确 Logs 主页 `exclude_sources=[test,quota]`、请求日志页相反）。漂移：规则引的 `proxy_log.rs:564`、`model_test.rs:157`、`quota/http.rs:187`、`useLogsFilters.ts:39` 四个行号需重取；`useLogsFilters` 已迁 `src/pages/Logs/useLogsFilters.ts` |
| `pagination-offset.md` | LIMIT+1 探测分页替代 COUNT(*) | **仍成立** | `db/proxy_log.rs:345` 注释「行只用于置位 has_more，Rust 侧截断，绝不下发给前端」；`:370 let has_more = items.len() > limit as usize` |
| `sqlite-partial-index.md` | 参数化绑定无法触发 partial index（仅认字面量） | **仍成立** | SQLite 规划器行为，与本仓代码解耦；结论有实测复现记录，可核 |
| `trellis-00.md` | DB 表设计规范（单数表名 / `"group"` 转义 / proxy_log TEXT uuid 无连字符） | **需改写** | 约束仍成立（`db/` 目录下表模块名 `group.rs`/`platform.rs`/`proxy_log.rs` 单数）。漂移：规则引 `src-tauri/src/gateway/db.rs`，现为 `crates/aidog_core/src/gateway/db/`（已拆成 40+ 文件） |
| `trellis-01.md` | tokio_rusqlite 连接韧性：`ConnectionClosed` 必须自动重连重试 1 次 | **需改写** | 机制完好：`db/mod.rs:89-91` 重连上下文注释、`:526 reopen_write_conn(&ctx.path)`、`:1031 async fn reopen_write_conn`。漂移：路径 `src-tauri/src/gateway/db/mod.rs` → `crates/aidog_core/src/gateway/db/mod.rs` |

---

## 6. recall/domain/（13 条，protected 2 条已排除）

| archive 内路径 | 一句话内容 | 判定 | 证据 |
|---|---|---|---|
| `bundled-models-fallback.md` | `include_str!` + `OnceLock` 只读兜底，DB 恒优先，禁启动 seed | **仍成立** | `db/model_price.rs:191-193` `None => price_sync::bundled_model_entry(model_name)` 正是「DB 查无→bundled 回退」，无 seed 路径 |
| `coding-plan-utilization-calib-fix-26.md` | coding plan 订阅制平台多无公开用量 API，先按 custom-quota-script 兜底 | **仍成立** | 领域事实（上游 ToS + API 可用性），非代码断言；无法从代码证伪也无需证伪 |
| `cpa-oauth-credential-format.md` | CLIProxyAPI OAuth 凭据格式与识别逻辑（`parse_oauth_json` / `is_oauth_credential`） | **需改写** | 函数都在：`gateway/cli_proxy_parser/parser.rs:196 parse_oauth_json`、`:387 fn is_oauth_credential`。漂移：模块名 `cpa_import` → `cli_proxy_parser`，规则里 4 处路径 + 行号全需重取 |
| `rule-51.md` | 5 协议锚点：只有 5 个 Protocol 可作 endpoint 协议，其余是平台别名 | **仍成立** | `gateway/models/protocol.rs` 枚举含 Mock(:21) / ClaudeCode(:24) / GlmCoding(:31) 等平台类型；前端 `src/domains/platforms/constants.ts:11 ENDPOINT_PROTOCOLS` 恰 5 条 |
| `rule-52.md` | `reasoning_content` → anthropic 出 text 块（方案 B），禁 thinking 块 | **仍成立** | 决策记录型规则，含否决理由（signature 缺失被 CC 多轮拒）+ 外部调研佐证，值得保留 |
| `rule-53.md` | N×N 互转走内部归一（路 A），非点对点 | **仍成立** | `adapter/converter/request.rs:12` + `response.rs:10` 是 parse/render 双向而非 N² 点对点函数 |
| `rule-54.md` | bug1 真相：`target_protocol` 落平台名的三层根因 | **需改写** | 结论并入 rule-05/07 更合适；单独看是历史 bug 复盘，`forward.rs:75` 行号已漂到 `:85-88`。建议并入 rule-07 作案例段 |
| `rule-55.md` | endpoint 跨协议回退三级，coding 平台不回退（401 防护） | **仍成立** | `gateway/proxy/endpoint.rs:95 select_endpoint_for_protocol` 仍是分层入口；「coding 平台永不落非 coding」与 CLAUDE.md 的 glm_coding 独立协议设计一致 |
| `rule-66.md` | `resolve_price` 末位 `now_ms` 各调用点传值约定 | **仍成立** | `db/model_price.rs:180-188` 签名末位确为 `now_ms: i64`；`billing.rs` / `estimate/db_ops.rs` / `platform_cmd/price.rs` 三类调用点均在 |
| `time-tiers-apply-idiom.md` | time_tiers 取 `start_at` 最大档、整体替换价表再嵌套 context 分档 | **仍成立** | 与 `rule-66` 同族且 CLAUDE.md「model-price-time-tiers」spec 已沉淀；`gateway/time_models.rs` 存在 |
| `trellis-06.md` | mock 平台类型规范（禁转发真实上游、按入站协议返、配置载体 `platform.extra`） | **需改写** | 核心断言全对：`proxy/handler.rs:412` `matches!(..., Protocol::Mock)` 短路、`protocol.rs:21 Mock`、`adapter/mock/config.rs:11 struct MockConfig`。漂移：规则写「`proxy.rs` 在 convert_request 之后拦截」，实际拦截点已迁到 `handler.rs:412`；`adapter/mock.rs` → `adapter/mock/config.rs` + `proxy/mock.rs` 两处 |
| `trellis-07.md` | Claude Code 订阅透传平台：bypass 所有转换，1:1 relay | **需改写** | `protocol.rs:24 ClaudeCode` 变体在；`proxy/passthrough.rs:25 handle_passthrough` 在。漂移：规则写 `proxy.rs handle_passthrough` / `models.rs`，实际为 `gateway/proxy/passthrough.rs` / `gateway/models/protocol.rs` |
| `trellis-08.md` | auto_disable 仅 401/403/402 触发，429 不触发 | **需改写** | 断言精确命中：`gateway/proxy/non_success.rs:68 if code == 401 \|\| code == 403 \|\| code == 402`，同行无 `is_429_quota_exhausted`。漂移：规则给的验收命令写 `src-tauri/src/gateway/proxy/non_success.rs`，路径需改 `crates/aidog_core/src/...` |
| `trellis-09.md` | `delete_platform` 软删平台 + 物理清关联 + 禁连带删组 | **需改写** | `db/platform_lifecycle.rs:29 fn delete_platform`、`:48 invalidate_groups_cache()`、`:47` 注释明确「展示为无成员卡片，与手动空组一致」。漂移：路径前缀同上 |
| `trellis-10.md` | 协议 logo 三路 fallback（simpleicons → favicon → clearbit），顺序禁重排 | **需改写** | `gateway/logo_sync.rs:95 async fn sync_one_into` 仍在。漂移：路径 `src-tauri/src/gateway/logo_sync.rs` → `crates/aidog_core/src/gateway/logo_sync.rs` |

---

## 7. recall/encoding · frontend · git · i18n（20 条）

| archive 内路径 | 一句话内容 | 判定 | 证据 |
|---|---|---|---|
| `encoding/trellis-21.md` | `<script type="application/json">` 内禁 HTML 实体转义，只防 `</script>` | **本就可疑** | 讲的是 server-side/build-time 模板（Python `html.escape` / `json.dumps`）嵌 JSON 到 HTML script 标签。本仓是 Tauri + Vite React，无此场景；grep 全仓无 `application/json` script 嵌入。疑似从别处带入的通用条目，在本仓无可验证锚点 |
| `frontend/auto-fix-downgrade-37.md` | macOS WKWebView HTML5 drop 不触发，必须用 Tauri `onDragDropEvent` | **仍成立** | `src/components/settings/ImportExport/ImportExportTab.tsx:77,272,278` 注释与实现均明确此限制并使用 `onDragDropEvent` |
| `frontend/cpa-drag-import-22.md` | `dragTargetRef` + HTML5 `onDragEnter` 标记区分拖入子区域（best-effort） | **已过期** | `grep -rn dragTargetRef src/` 零命中；CPA 导入 UI 已重构为 `src/pages/CliProxy/ImportDialog.tsx`，无该模式 |
| `frontend/cpa-drag-import-23.md` | 多源批量导入 `orderLenRef` baseIdx 偏移保 rowId 唯一 | **已过期** | `grep -rn orderLenRef src/` 零命中；`ImportDialog.tsx` 无 rowId/baseIdx 逻辑 |
| `frontend/cpa-drag-import-24.md` | `parseInFlightRef` 计数替代 boolean loading | **已过期** | `grep -rn parseInFlightRef src/` 零命中 |
| `frontend/dirty-float-hour-normalization.md` | 浮点 hour 脏数据在前端 parse 层单点归一 | **需改写** | 内核成立且高价值（半时区历史数据）。锚点在：`src/utils/peakHours.ts`、`src/services/api/platforms.ts` 存在。漂移：规则 frontmatter 里 `src-tauri/crates/aidog_core/gateway/time_models.rs` 路径缺 `src/` 段，实际 `crates/aidog_core/src/gateway/time_models.rs` |
| `frontend/form-level-tz-state-sharing.md` | 表单级单一 `tzMode` state 透传，禁各组件独立 | **仍成立** | `src/pages/platforms/{PlatformEditForm.tsx,usePlatformForm.ts,ModelsMatrixSection.tsx,formSections.tsx}` 四个锚点文件全部存在 |
| `frontend/modal-state-architecture.md` | 两类 Modal 的 state 归属（直接灌表单 vs 跨表单） | **需改写** | 「直接灌表单」一类完全对得上：`usePlatformForm.ts:161 const [showPaste, setShowPaste]`、`platformPasteApply.ts:20 PlatformPasteCtx`。但「跨表单」举的 `CpaImportModal` 已不存在（改为 `src/pages/CliProxy/ImportDialog.tsx`），需换例 |
| `frontend/platform-creation-entry-consolidation.md` | cli-proxy 平台唯一创建入口 = CliProxy 页按钮 | **仍成立** | `src/pages/CliProxy/index.tsx:180 await cliProxyApi.createPlatform(p.id)`；`src/pages/platforms/PlatformEditForm.tsx` 无 cli-proxy 创建旁路 |
| `frontend/semantic-token-foreground-pairing.md` | 语义色 token 必须配达标 `-foreground`，禁改 `--accent` 本值 | **仍成立** | 本项目特定约束；`src/styles/globals.css` + `src/themes/` 主题体系未变 |
| `frontend/shadcn-infra-30.md` | CSS 变量改名用 `:root` 别名层做 live resolution | **仍成立** | 技巧型规则，无锚点依赖；`globals.css` 仍是 CSS 变量驱动 |
| `frontend/shadcn-infra-31.md` | shadcn token 运行时切换用 `setProperty` inline，免 `!important` | **仍成立** | `src/themes/index.ts:16 document.documentElement.setAttribute("data-mode", mode)` + inline style 写入，与规则一致 |
| `frontend/tailwind-cascade-layer-unlayered.md` | 分层导入下裸写规则反压 layer 内 utility，UA reset 必须包 `@layer base` | **仍成立** | `globals.css:4` `@layer theme, base, components, utilities;`、`:5-6` 分层 `@import ... layer(...)`；`:10` 与 `:29` 两处红字注释「🔴 必须写在 `@layer base` 内」，`:14`/`:33` 实际用 `@layer base {` 包裹 |
| `frontend/theme-dark-class-dead-code.md` | 本项目 `data-mode` 驱动，`dark:` utility 是死代码 | **仍成立** | `src/themes/index.ts:16` 只 `setAttribute("data-mode")`，无 `classList.add("dark")`；规则点名的两处残留仍在原行号：`src/components/ui/alert.tsx:13`、`src/components/ui/field.tsx:120` |
| `frontend/theme/shadcn-primitives-40.md` | shadcn Sonner 导入 next-themes 与自有主题体系冲突（待决策） | **已过期** | `src/components/ui/sonner.tsx` 已不存在；`grep -rn next-themes src/` 零命中。冲突已随组件删除消解（`package.json:63,71` 残留未用依赖是另一回事） |
| `frontend/time-zone-minute-arithmetic.md` | 时区换算走绝对分钟 modulo 1440，hour+minute 成对处理 | **仍成立** | `src/utils/peakHours.ts` + `src/services/api/platforms.ts` 均在；与 `dirty-float-hour-normalization` 同族，高价值 |
| `frontend/trellis-18.md` | 前端目录/组件/hook/CRUD 刷新链约定 | **需改写** | 多条已漂：①「服务层 API 放 `src/services/api.ts`」→ 现为 `src/services/api/` 目录；②「共享组件放 `src/components/`，禁嵌套 >1 层」→ 现有 `src/components/settings/editors/StatusLineSection/` 三层嵌套，且已成型 `src/domains/{platforms,groups,shared}/` 域层，规则里完全没提。仍成立的部分：新页面放 `src/pages/`、主题放 `src/themes/`、i18n 放 `src/locales/`（均已核） |
| `git/rule-44.md` | 并行 subtask commit 前用 `git diff --cached --name-only` 核验落点 | **仍成立** | 流程纪律，与 skein 并行执行模式配套 |
| `i18n/i18n-key-deletion-safety.md` | 删 i18n key 前逐 key grep 确认引用归零 | **仍成立** | `scripts/check-i18n.mjs` 存在 |
| `i18n/rule-04.md` | 新增 i18n key 必须同步 8 语言 | **仍成立** | `ls src/locales/` 恰 8 个 json（ar-SA/de-DE/en-US/es-ES/fr-FR/ja-JP/ru-RU/zh-Hans）+ index.ts |
| `i18n/trellis-19.md` | locale 标签必须 `zh-Hans`（BCP47 script），禁 `zh-CN`；三层字面同集 | **需改写** | 结论仍成立（`src/locales/zh-Hans.json` 在，无 zh-CN）。漂移：规则引 `src/domains/platforms/defaults.ts:103-105` 桥接层删除位置、`src-tauri/src/gateway/i18n.rs`（现 `crates/aidog_core/src/gateway/i18n.rs`），行号/路径需重取 |

---

## 8. recall/ops · optimization · proxy · reuse（15 条，protected 12 条已排除）

| archive 内路径 | 一句话内容 | 判定 | 证据 |
|---|---|---|---|
| `ops/buf-residue-observability.md` | 缓冲残留禁静默丢，必须留可观测信号 | **仍成立** | `gateway/proxy/stream.rs:97,153,272` 三处超上界均带 `warn!`（非静默 clear），与规则一致 |
| `ops/buffer-residue-no-silent-drop.md` | 同上，仅标题一行无正文 | **本就可疑（重复组）** | 全文只有 frontmatter + `## 缓冲残留处置·禁静默丢` 一行。与上一条同题，空壳。**这是第三组重复**（brief 只点了两组） |
| `ops/tauri-logging-guard-lifecycle.md` | `tracing_appender::non_blocking` 的 WorkerGuard 必须绑 app 生命周期（`app.manage()`） | **需改写** | 机制成立且有完整备选否决表。漂移：锚点 `src-tauri/src/startup.rs` 仍在，但需核对 guard 现在的持有位置（本轮未逐行核到 `app.manage(guard)`，改写时补证） |
| `ops/trellis-17.md` | 远端 defaults JSON 同步 7 件套（双源 fetch + `last_updated` 比对 + 节流 + 校验） | **需改写** | 实现在：`gateway/defaults_sync.rs:22` jsDelivr 主源、`:26` raw.githubusercontent fallback、`:43` 写盘来源三态 `"jsdelivr"\|"raw"\|"local"`。漂移：路径前缀 `src-tauri/src/gateway/` → `crates/aidog_core/src/gateway/`；`client_types_sync.rs` 仍在（第 2 实例成立） |
| `optimization/api-payload-optimization.md` | 后端 `SELECT DISTINCT` 替代前端 Set 去重降 IPC payload | **仍成立** | `gateway::db::distinct_models_proxy_log(&db, &filter, actual, limit)` 存在（`proxy_cmd/test_proxy_log.rs:28` 调用可证） |
| `optimization/manual-budget-empty-shortcircuit.md` | 零配额走只读池预检短路，不进写连接 | **仍成立** | `gateway/manual_budget.rs:189 fn has_any_budget`、`:218 if !has_any_budget(...).await? {` 短路，行号仅漂 0~7 行 |
| `proxy/router.md` | `sole_platform` 判定两分支 + 正交维度（expires_at / disable_during_peak）不参与 | **仍成立** | `gateway/router/mod.rs:98 pub(crate) fn sole_platform`，行号未漂；正交维度说明与 CLAUDE.md「disable_during_peak 独立维度」一致 |
| `proxy/rule-50.md` | proxy_log 异步日志队列方案 B（单 writer + 有界队列 + 分级背压） | **需改写** | 实现在：`gateway/proxy/mod.rs:197 pub(crate) const LOG_QUEUE_CAP: usize = 512`、`:242` mpsc channel。漂移：规则引 `src/gateway/proxy/log.rs` / `mod.rs`，需加 `crates/aidog_core/` 前缀 |
| `proxy/sse-chunk-stateless-defect.md` | 逐 chunk 无状态 SSE 解析致整行静默丢失，须尾行缓冲 + 无状态解析分离 | **仍成立** | `gateway/proxy/stream.rs:133 pub(crate) struct SseLineReassembler` 正是规则所指的重组器；`:130` 注释明确与 `feed_sse_usage` 同口径 |
| `proxy/trellis-11.md` | CONNECT 隧道：禁 axum `.route()` 注册，在 handler 头部按 method 早期分流 | **需改写** | `gateway/proxy/connect.rs:50 pub(crate) async fn handle_connect` 在，机制成立。漂移：路径前缀 + `handler.rs` 分流行号需重取 |
| `proxy/trellis-12.md` | `should_fallback_passthrough` 的 host 判定必须前置于 path 判定 | **仍成立** | `gateway/proxy/handler.rs:256 if should_fallback_passthrough(host_header, state.listen_addr...)`，仍是 host 优先的单参判定；`gateway/proxy/mod.rs:87` 导出 |
| `proxy/trellis-13.md` | absolute-form 请求在 Router 顶层 middleware 识别，禁 `.route()` | **仍成立** | `gateway/proxy/mod.rs:328` 中间件挂载、`:340 async fn absolute_form_forward_mw`，与规则描述完全一致 |
| `proxy/trellis-14.md` | `build_http_client` 的 `use_proxy=false` 分支必须 `.no_proxy()`，防 CONNECT 递归环 | **仍成立** | `gateway/logo_sync.rs:12` 注释直接引此约束「复用 build_http_client（禁 env proxy 防 forward 递归环，见 http_client.rs 注释）」—— 规则已被代码注释背书 |
| `proxy/trellis-15.md` | 诊断 header 统一走 `headers.rs::inject_trace_header`，禁各点重复 cfg gate | **仍成立** | `gateway/proxy/notify.rs:48,59` 调用 `inject_trace_header(&mut r)`，helper 复用模式在用 |
| `reuse/auto-fix-downgrade-36.md` | 写代码前 grep 查复用；新增协议/主题/locale 的注册清单 | **需改写** | 「写前 grep」内核永远成立。漂移：清单里「新增平台协议必须扩展 `Protocol` union + `PROTOCOLS` 数组」已不准 —— 前端 `PROTOCOLS` 现由 `src/domains/platforms/defaults.ts` 从 JSON 派生（见 `trellis-05`），只有 `ENDPOINT_PROTOCOLS` 5 条仍硬编码（`constants.ts:11`） |

---

## 9. recall/shadcn · style · test · testing · ts-rust-boundary（13 条）

| archive 内路径 | 一句话内容 | 判定 | 证据 |
|---|---|---|---|
| `shadcn/rule-03.md` | Radix Dialog 必须含 DialogTitle，自定义 header 用 `sr-only` | **仍成立** | `src/components/settings/editors/StatusLineSection/SegmentEditModal.tsx:50-51` 注释 + `<DialogTitle className="sr-only">`，行号仅漂 1~2 行 |
| `shadcn/rule-41.md` | radix Select 空值用 `__none__` 哨兵 | **需改写** | 模式仍在用：`src/domains/groups/PlatformPicker.tsx:105-109` 注释「radix Select 禁 value="" → `__none__` 哨兵映射回 0」。漂移：规则引的两个锚点 `EnvEditor.tsx:55` / `Logs/primitives.tsx:12` 需重取（文件在，行号未验），主用例应换 PlatformPicker |
| `shadcn/rule-42.md` | radix Select number 双向 String()/Number() 映射 | **仍成立** | `src/pages/Logs/primitives.tsx` 存在；radix API 约束未变 |
| `shadcn/rule-43.md` | `Dialog.open` 需 `modalState !== null` 显式判断 | **仍成立** | Radix API 约束，无锚点依赖 |
| `shadcn/rule-45.md` | 只读域（TrayConfigTab）跳过 shadcn 迁移，planning 先 grep 预筛 | **需改写** | `src/pages/TrayConfigTab.tsx` 仍在。但 shadcn 迁移已完成（`src/components/ui/` 25 个组件齐备），这条的价值从「迁移范围判定」退化为「planning 先 grep 预筛范围」这一通用纪律。建议提炼为通用条目，删 shadcn 迁移语境 |
| `shadcn/rule-46.md` | shadcn Button cva 基类 `[&_svg]:size-4` 强压 16px | **仍成立** | `src/components/ui/button.tsx:8` cva 基类内确含 `[&_svg]:size-4`，行号未漂 |
| `shadcn/rule-47.md` | dnd-kit SortableList 迁移只换视觉，保留拖拽逻辑 | **仍成立** | `src/components/SortableList.tsx` 在；`package.json:24-26` dnd-kit 三包在 |
| `style/css-reset-layer.md` | CSS reset 必须写进 `@layer base` | **需改写（与 tailwind-cascade-layer-unlayered 高度重叠）** | 结论同 `frontend/tailwind-cascade-layer-unlayered`（证据同 `globals.css:4-6,10,14,29,33`）。两条讲同一个 cascade layer 陷阱，只是切入点不同（reset vs UA reset）。建议合并为一条，保留 `tailwind-cascade-layer-unlayered`（判据更严谨、有 commit 级案例 c3f9515e→ce3d5dd5） |
| `style/trellis-16.md` | 日志 5 段字段顺序 + msg 段禁丢业务字段 + traceid 取值链 | **需改写** | `src-tauri/src/logging.rs` 仍在（app 层未迁 crate）；但规则里 `src-tauri/src/gateway/proxy/health.rs` 已迁 `crates/aidog_core/src/gateway/proxy/health.rs`，路径需分别处理（logging.rs 留 app 层、gateway 侧迁 crate） |
| `test/rule-48.md` | shadcn 迁移后测试改行为断言而非 className/snapshot | **仍成立** | 测试纪律，与 `src/pages/platforms/usePlatformsState.test.ts` 等现存测试风格一致 |
| `test/rule-65.md` | 迁入 aidog_core 的测试文件须 `aidog_core::` → `crate::` | **需改写** | Rust 路径规则本身永远成立，但「迁入 aidog_core」这一次性迁移已完成（commands_* 已全部并入）。改写为通用条目「测试文件跨 crate 移动时改全限定路径为 `crate::`」，删 arch-deepen-2 案例 |
| `testing/deterministic-pseudorandom-loadgen.md` | 压测确定性伪随机 = 原子计数器 + splitmix64 乘法哈希，不引 rand | **仍成立** | `gateway/proxy/mock.rs:5` 注释 + `:14 n.wrapping_mul(0x9E3779B97F4A7C15)`，实现与规则逐字对应 |
| `testing/module-load-time-constant-test-rule.md` | 模块加载期求值的常数须拆纯函数内核参数化才可单测 | **仍成立** | `src/utils/peakHours.ts` 在；`vi.spyOn` 对模块常数无效是 vitest/ESM 语义，结论稳定 |
| `ts-rust-boundary/mock-config-4layer-consistency.md` | mock 配置四层（Rust struct / 序列化 / TS 类型 / 反序列化）字段须同步 | **需改写** | 四层都在：`gateway/adapter/mock/config.rs:11 pub struct MockConfig`；`parseMockConfig`/`serializeMockConfig` 在 `src/services/api/`（`usePlatformForm.ts:12` import 可证）。漂移：规则写 `config.rs:11-25` / `manual.ts:467` / `platforms.ts:124`，模块路径全变（`proxy/config.rs` → `adapter/mock/config.rs`） |
| `ts-rust-boundary/optional-config-backward-compat.md` | `Option<T>` 新旋钮与旧字段共存（`ttft_ms`/`inter_chunk_ms` vs `delay_ms`） | **需改写** | 实现完好：`gateway/proxy/mock.rs:37 cfg.ttft_ms.unwrap_or(cfg.delay_ms)`、`:132 cfg.inter_chunk_ms.unwrap_or(cfg.delay_ms)` —— 正是规则描述的兼容取值。漂移：`config.rs:11` 路径同上条 |

---

## 10. rules/（4 条）

| archive 内路径 | 一句话内容 | 判定 | 证据 |
|---|---|---|---|
| `rules/arch/mock-platform-bypasses-forward-pipeline.md` | mock 平台短路绕开真实转发流水线，压测验证 cap/累积必须用非 mock | **需改写** | 断言正确：`gateway/proxy/handler.rs:412` 短路（规则写 `handler.rs:410-429`，仅漂 2 行）；`finish.rs:280` 注释确认 `STREAM_BODY_MAX_BYTES` 挂在真实路径。`anchors:` 三条路径全部有效。仅需微调行号 |
| `rules/perf/hot-path-buffers.md` | mpsc 丢弃分支先查 `capacity()==0` 再决定深拷贝；热点判定看频次非字节量 | **仍成立** | `anchors: crates/aidog_core/src/gateway/proxy/log.rs` 有效；`gateway/proxy/mod.rs:197 LOG_QUEUE_CAP=512` 是配套有界队列 |
| `rules/perf/stream-buf-no-batching.md` | 流缓冲只留不完整尾巴，完整帧立刻下发，禁攒批 | **仍成立** | `gateway/proxy/stream.rs:133 SseLineReassembler` 逐行 split 下发；内容完整（有 MUST / 触发 / 关联） |
| `rules/perf/stream-buffer-no-batching-delay.md` | 同上，仅标题一行无正文 | **本就可疑（重复组）** | 全文只有 frontmatter + `## 流缓冲不得攒批` 一行，空壳 |

---

## 11. 重复组处置建议

| 组 | 成员 | 建议 |
|---|---|---|
| A. 流缓冲上界单一真值源 | `core/arch/stream-buf-unified-cap.md`（完整：硬约束+MUST+验收+正反例+关联） / `core/arch/stream-buffer-cap-single-source.md`（空壳，1 行） | **保留 stream-buf-unified-cap**，弃空壳 |
| B. 流缓冲不得攒批 | `rules/perf/stream-buf-no-batching.md`（完整） / `rules/perf/stream-buffer-no-batching-delay.md`（空壳，1 行） | **保留 stream-buf-no-batching**，弃空壳 |
| C. 缓冲残留禁静默丢 ⚠️ brief 未点出 | `recall/ops/buf-residue-observability.md`（完整：根因+调试路径分析） / `recall/ops/buffer-residue-no-silent-drop.md`（空壳，1 行） | **保留 buf-residue-observability**，弃空壳 |
| D. shadcn add 后验证依赖 | `recall/build/shadcn-infra-28.md`（有 `yarn why` 验收命令） / `recall/build/shadcn/shadcn-primitives-39.md`（有具体缺失包清单） | **合并为一条**，以 `-28` 为底稿，把 `-39` 的包清单并入 |
| E. cascade layer / reset 分层 | `recall/frontend/tailwind-cascade-layer-unlayered.md`（判据严谨 + commit 级案例） / `recall/style/css-reset-layer.md`（同结论，切入点为 reset） | **合并**，保留 `tailwind-cascade-layer-unlayered`，把 reset 场景并为其一个案例 |
| F. 拆库访问点审计 | `recall/arch/auto-fix-downgrade-34.md`（三形式） / `recall/arch/non-typical-sql-audit-pattern.md`（两形式，是前者子集） | **合并**，保留 `auto-fix-downgrade-34`（覆盖更全） |

> 空壳规则（A/B/C 组的第二份）三条形态完全一致：只有 frontmatter + 一个 `##` 标题行。是本会话 specer 批量造成的同题空文件，非内容分歧。

---

## 12. `trellis-NN` / `rule-NN` 批次核查结论（brief 特别点名）

**结论与预期相反：这批并非「最可能已过期」，绝大多数内核仍成立，问题集中在路径漂移。**

- `trellis-NN`（22 条）：仅 `trellis-03`（workspace crate 边界）真过期；`trellis-21`（HTML JSON 嵌入）本就与本仓无关；其余 20 条内核全部成立，但**几乎全部**引用 `src-tauri/src/gateway/**` 旧路径，需批量改写为 `src-tauri/crates/aidog_core/src/gateway/**`。
- `rule-NN`（27 条）：无一整条过期。`rule-60/62/63/65` 因 commands_* 合并而语境失效（需改写为通用条目），`rule-54` 建议并入 `rule-07`，`rule-45` 需去 shadcn 迁移语境。其余锚点核到的（`rule-46` button.tsx:8、`rule-57` protocol.rs:173、`rule-58` forward.rs:85、`rule-64` command_macro.rs:12、`rule-03` SegmentEditModal.tsx:50）**行号都没漂或只漂 1~2 行**，质量意外地高。

---

## 13. 重建建议汇总（仍成立 + 需改写两档）

### 建议进 `core/`（硬约束命令式，违反即 bug / 数据损坏）

| 主题 | 拟 namespace/category | 来源 | 改写要点 |
|---|---|---|---|
| 流缓冲上界单一真值源 | core/arch | A 组胜出者 | 原样保留，删空壳同题文件 |
| 流缓冲不得攒批 | core/perf | B 组胜出者 | 从 `rules/perf/` 提到 core（是硬约束不是经验） |
| SQLite 只读缓存定值 | core/db | `sqlite-read-cache-config` | 原样 |
| peak 倍率对边对称 | core/domain | `rule-67` | 行号 214/233→219/237，函数名改 `peak_hours::resolve_multiplier` |
| `resolve_price` now_ms 传值约定 | core/domain | `rule-66` | 原样（签名已核） |
| auto_disable 仅 401/403/402 | core/proxy | `trellis-08` | 路径加 `crates/aidog_core/` 前缀，验收 grep 更新 |
| `delete_platform` 禁连带删组 | core/domain | `trellis-09` | 同上路径前缀 |
| `build_http_client` 禁 env proxy | core/proxy | `trellis-14` | 同上；代码注释已背书，可引 `logo_sync.rs:12` |
| CONNECT / absolute-form 禁 `.route()` | core/proxy | `trellis-11` + `trellis-13` | 两条可合并为「非标准 URI 形态禁走 axum path matcher」 |
| host 判定前置于 path 判定 | core/proxy | `trellis-12` | 更新 `handler.rs:256` 行号 |
| 新增 wire protocol 三处同步 | core/proxy | `rule-05` | converter 路径补 `adapter/` 段 |
| Rust↔TS 边界契约（snake_case + invoke 包装） | core/cross-layer | `trellis-20` | `src/services/api.ts` → `src/services/api/` 目录，全篇路径重写 |
| `sole_platform` 判定 Rust↔TS 对称 | core/cross-layer | `ts-rust-symmetry` + `proxy/router` | 两条同题可合并 |
| 拆库 crash-safe 四阶段 | core/db | `crash-safe-db-split-migration` | 原样 |
| `ConnectionClosed` 必须重连重试 1 次 | core/db | `trellis-01` | 路径前缀 + 行号（`db/mod.rs:526,1031`） |
| i18n key 必须同步 8 语言 | core/i18n | `rule-04` | 原样 |
| `zh-Hans` 三层字面同集 | core/i18n | `trellis-19` | 行号重取 |
| UA reset / 全局元素规则必须包 `@layer base` | core/frontend | E 组合并后 | 保留 `tailwind-cascade-layer-unlayered` 为底稿 |
| `dark:` utility 在本项目是死代码 | core/frontend | `theme-dark-class-dead-code` | 原样（两处残留行号已复核） |
| mock 平台绕开转发流水线 | core/arch | `mock-platform-bypasses-forward-pipeline` | `handler.rs:410-429` → `:412` |

### 建议进 `recall/`（长尾经验 / 陷阱 / 方法）

按 category 归拢，改写要点见各表「证据」列：

- **recall/arch**：`auto-fix-downgrade-33/34(+non-typical 合并)/35/38`、`rule-49`(popover 复用)、`rule-56`(alt=sse 行号)、`rule-57`、`rule-58`、`rule-59`、`rule-60`(去搬迁语境)、`rule-62`(去搬迁语境)、`rule-64`、`shadcn-infra-32`、`trellis-04`(删 `default_is_coding_plan`)、`trellis-05`、`coding-plan-utilization-calib-fix-25`、`cross-db-subquery-handle-selection`、`parser-multi-path-format-symmetry`(改 `cli_proxy_parser`)
- **recall/build**：`rule-06`、`rule-07`(并入 rule-54 案例)、`rule-61`、`rule-63`(去 commands_tray 案例)、`shadcn-infra-02`、`shadcn-infra-28`(合并 -39)、`shadcn-infra-29`、`tauri-build-bundle`、`trellis-02`(降为 workspace 卫生规则)
- **recall/db**：`filter-semantics`(行号重取)、`pagination-offset`、`sqlite-partial-index`、`trellis-00`(路径改 db/ 目录)
- **recall/domain**：`bundled-models-fallback`、`coding-plan-utilization-calib-fix-26`、`cpa-oauth-credential-format`(改 `cli_proxy_parser`)、`rule-51`、`rule-52`、`rule-53`、`rule-55`、`time-tiers-apply-idiom`、`trellis-06`(拦截点改 handler.rs)、`trellis-07`(路径)、`trellis-10`(路径)
- **recall/frontend**：`auto-fix-downgrade-37`、`dirty-float-hour-normalization`(补 `src/` 路径段)、`form-level-tz-state-sharing`、`modal-state-architecture`(换跨表单例)、`platform-creation-entry-consolidation`、`semantic-token-foreground-pairing`、`shadcn-infra-30`、`shadcn-infra-31`、`time-zone-minute-arithmetic`、`trellis-18`(补 domains 层 + api 目录)
- **recall/ops**：`buf-residue-observability`(C 组胜出)、`tauri-logging-guard-lifecycle`(补 guard 持有点证据)、`trellis-17`(路径前缀)
- **recall/optimization**：`api-payload-optimization`、`manual-budget-empty-shortcircuit`(行号 189/218)
- **recall/proxy**：`rule-50`(路径前缀)、`sse-chunk-stateless-defect`、`trellis-15`
- **recall/reuse**：`auto-fix-downgrade-36`(删「PROTOCOLS 数组」条，改为 defaults.ts 派生)
- **recall/shadcn**：`rule-03`、`rule-41`(换 PlatformPicker 例)、`rule-42`、`rule-43`、`rule-45`(提炼为通用预筛纪律)、`rule-46`、`rule-47`
- **recall/style**：`trellis-16`(logging.rs 留 app 层 / gateway 侧迁 crate)
- **recall/test**：`rule-48`、`rule-65`(去 arch-deepen-2 语境)
- **recall/testing**：`deterministic-pseudorandom-loadgen`、`module-load-time-constant-test-rule`
- **recall/ts-rust-boundary**：`mock-config-4layer-consistency`(路径 `adapter/mock/config.rs`)、`optional-config-backward-compat`(同上)
- **recall/git**：`rule-44`
- **recall/i18n**：`i18n-key-deletion-safety`

---

## 14. 统计

| 判定 | 条数 | 占比 |
|---|---:|---:|
| **仍成立** | 66 | 58.4% |
| **需改写** | 38 | 33.6% |
| **已过期** | 5 | 4.4% |
| **本就可疑** | 4 | 3.5% |
| **合计** | **113** | 100% |

分节明细（校验用）：

| 节 | 条数 | 仍成立 | 需改写 | 已过期 | 本就可疑 |
|---|---:|---:|---:|---:|---:|
| 1 core/ | 4 | 2 | 1 | 0 | 1 |
| 2 recall/arch | 20 | 13 | 6 | 1 | 0 |
| 3 recall/build | 11 | 8 | 3 | 0 | 0 |
| 4 recall/cross-layer | 2 | 1 | 1 | 0 | 0 |
| 5 recall/db | 6 | 3 | 3 | 0 | 0 |
| 6 recall/domain | 15 | 8 | 7 | 0 | 0 |
| 7 encoding/frontend/git/i18n | 21 | 12 | 4 | 4 | 1 |
| 8 ops/optimization/proxy/reuse | 15 | 9 | 5 | 0 | 1 |
| 9 shadcn/style/test/testing/ts-rust | 15 | 8 | 7 | 0 | 0 |
| 10 rules/ | 4 | 2 | 1 | 0 | 1 |
| **合计** | **113** | **66** | **38** | **5** | **4** |

**已过期 5 条**：`recall/arch/trellis-03`（commands_* crate 不存在）、`recall/frontend/cpa-drag-import-22/23/24`（三个 ref 模式全部 grep 零命中）、`recall/frontend/theme/shadcn-primitives-40`（sonner.tsx 已删）

**本就可疑 4 条**：`core/arch/stream-buffer-cap-single-source`、`recall/ops/buffer-residue-no-silent-drop`、`rules/perf/stream-buffer-no-batching-delay`（三个空壳）、`recall/encoding/trellis-21`（本仓无该场景）

> `recall/domain/rule-54` 判为「需改写」（并入 `rule-07` 作案例段），不计入可疑档。

**重复组：6 组**（brief 点了 2 组，实际发现 6 组；其中 C 组是与 A/B 同形态的第三个空壳对，D/E/F 是内容重叠对）

**「需改写」的主因分布**：路径前缀漂移（`src-tauri/src/gateway/` → `crates/aidog_core/src/gateway/`）约 20 条，是最大的单一原因；其次是 commands_* crate 合并导致的语境失效（5 条）、模块改名 `cpa_import`→`cli_proxy_parser`（2 条）、前端 `api.ts`→`api/` 目录（2 条）。**这些都是机械可修的，不影响规则内核。**

---

## 15. 三态处置表（keep / rewrite / drop）+ slug 决策

**与第 14 节四档的换算**：四档是「规则内容与代码是否对得上」的**事实判定**；三态是「重建时怎么办」的**动作决策**。差异只出在合并组的被并方 —— 它们内容不假（判定可能是仍成立/需改写），但动作是 drop（并入胜出者）。

| 换算 | 条数 |
|---|---:|
| keep = 仍成立 66 − 被并方 1（`non-typical-sql-audit-pattern`） | **65** |
| rewrite = 需改写 38 − 被并方 3（`css-reset-layer` / `shadcn-primitives-39` / `rule-54`） | **35** |
| drop = 已过期 5 + 本就可疑 4 + 被并方 4 | **13** |
| 合计 | **113** |

**slug 统计**：编号 slug 共 73 条（`trellis-NN` 22 + `rule-NN` 32 + `auto-fix-downgrade-NN` 6 + `shadcn-infra-NN` 6 + `shadcn-primitives-NN` 2 + `cpa-drag-import-NN` 3 + `coding-plan-utilization-calib-fix-NN` 2）。其中 8 条落 drop 无需改名，**65 条需 rewrite-slug**。

**namespace 归位**：`rules/` 非标准 namespace（标准仅 core / recall），4 条全部归位 —— 3 条进 `core/`，1 条 drop。

备注列标记：`[路径]` = 仅需批量替换 `src-tauri/src/gateway/` → `src-tauri/crates/aidog_core/src/gateway/`，内容不动。

### core/（4）

| archive 路径 | 三态 | slug 处置 | 备注 |
|---|---|---|---|
| `core/arch/stream-buf-unified-cap` | keep | keep-slug | — |
| `core/arch/stream-buffer-cap-single-source` | **drop** | — | 空壳重复（采信 recon-backend） |
| `core/db/sqlite-read-cache-config` | keep | keep-slug | — |
| `core/domain/rule-67` | rewrite | rewrite-slug → `peak-multiplier-symmetry` | 行号 214/233→219/237；函数名改 `peak_hours::resolve_multiplier`。新名取自其 frontmatter 既有 `name:` |

### recall/arch（20）

| archive 路径 | 三态 | slug 处置 | 备注 |
|---|---|---|---|
| `auto-fix-downgrade-33` | keep | rewrite-slug → `agent-platform-handler-branch` | — |
| `auto-fix-downgrade-34` | rewrite | rewrite-slug → `db-split-access-point-audit` | 合并 `non-typical-sql-audit-pattern`（胜出方） |
| `auto-fix-downgrade-35` | keep | rewrite-slug → `dedup-key-must-be-nonempty` | — |
| `auto-fix-downgrade-38` | keep | rewrite-slug → `enum-variant-delete-needs-migration` | — |
| `coding-plan-utilization-calib-fix-25` | rewrite | rewrite-slug → `coding-plan-base-url-from-endpoint` | 定位改 endpoint 解析处 |
| `cross-db-subquery-handle-selection` | keep | keep-slug | — |
| `non-typical-sql-audit-pattern` | **drop** | — | 是 `-34` 的子集，并入 |
| `parser-multi-path-format-symmetry` | rewrite | keep-slug | `cpa_import`→`cli_proxy_parser`，4 处路径重取 |
| `rule-49` | keep | rewrite-slug → `tauri-popover-window-reuse` | — |
| `rule-56` | rewrite | rewrite-slug → `gemini-sse-alt-param` | 行号 203-211→309-311 |
| `rule-57` | keep | rewrite-slug → `protocol-wire-str` | — |
| `rule-58` | keep | rewrite-slug → `adapter-deadcode-whitelist-authority` | — |
| `rule-59` | keep | rewrite-slug → `component-extraction-grep-callsites` | — |
| `rule-60` | rewrite | rewrite-slug → `invoke-name-source-of-truth` | 触发场景改「command 增删改名」，去跨 crate 搬迁语境 |
| `rule-62` | rewrite | rewrite-slug → `i18n-key-set-diff-check` | 同上去搬迁语境 |
| `rule-64` | keep | rewrite-slug → `tauri-command-macro-no-mut` | — |
| `shadcn-infra-32` | keep | rewrite-slug → `locale-deadkey-cleanup-ownership` | — |
| `trellis-03` | **drop** | — | commands_* crate 已不存在 |
| `trellis-04` | rewrite | rewrite-slug → `protocol-variant-extension` | 删已不存在的 `default_is_coding_plan` 条 |
| `trellis-05` | keep | rewrite-slug → `frontend-constants-derived-from-json` | — |

### recall/build（11）

| archive 路径 | 三态 | slug 处置 | 备注 |
|---|---|---|---|
| `rule-05` | keep | rewrite-slug → `wire-protocol-whitelist-sync` | converter 路径补 `adapter/` 段 |
| `rule-06` | keep | rewrite-slug → `converter-endpoint-decoupled` | — |
| `rule-07` | rewrite | rewrite-slug → `wire-protocol-gate-is-failfast` | 吸收 `rule-54` 作案例段 |
| `rule-61` | keep | rewrite-slug → `clippy-touch-before-recheck` | — |
| `rule-63` | rewrite | rewrite-slug → `build-rs-env-is-crate-scoped` | 删 commands_tray 案例 |
| `shadcn-infra-02` | keep | rewrite-slug → `tailwind-v4-import-form` | — |
| `shadcn-infra-28` | rewrite | rewrite-slug → `shadcn-add-verify-deps` | 合并 `-39` 的缺失包清单（胜出方） |
| `shadcn-infra-29` | keep | rewrite-slug → `vite-at-alias-manual` | — |
| `shadcn/shadcn-primitives-39` | **drop** | — | 与 `-28` 同题，并入（采信 recon-backend 同类判断） |
| `tauri-build-bundle` | keep | keep-slug | — |
| `trellis-02` | rewrite | rewrite-slug → `cargo-workspace-hygiene` | 删 PoC 迁移流程，只留 `[workspace.dependencies]` 对齐 + binary 同名两条 |

### recall/cross-layer（2）+ recall/proxy/router

| archive 路径 | 三态 | slug 处置 | 备注 |
|---|---|---|---|
| `cross-layer/trellis-20` | rewrite | rewrite-slug → `tauri-react-boundary-contract` | `src/services/api.ts` → `src/services/api/` 目录，全篇路径重写 |
| `cross-layer/ts-rust-symmetry` | rewrite | rewrite-slug → `sole-platform-symmetry` | 与 `proxy/router` 合并为一条（胜出方，落 cross-layer） |
| `proxy/router` | rewrite | 并入上条 | Rust 侧口径段并入 `sole-platform-symmetry` |

### recall/db（6）

| archive 路径 | 三态 | slug 处置 | 备注 |
|---|---|---|---|
| `crash-safe-db-split-migration` | keep | keep-slug | — |
| `filter-semantics` | rewrite | keep-slug | 4 个行号重取；`useLogsFilters` 已迁 `src/pages/Logs/` |
| `pagination-offset` | keep | keep-slug | — |
| `sqlite-partial-index` | keep | keep-slug | — |
| `trellis-00` | rewrite | rewrite-slug → `db-table-conventions` | `gateway/db.rs` → `gateway/db/` 目录 |
| `trellis-01` | rewrite | rewrite-slug → `sqlite-connection-resilience` | [路径] + 行号 `db/mod.rs:526,1031` |

### recall/domain（15）

| archive 路径 | 三态 | slug 处置 | 备注 |
|---|---|---|---|
| `bundled-models-fallback` | keep | keep-slug | — |
| `coding-plan-utilization-calib-fix-26` | keep | rewrite-slug → `coding-plan-no-public-quota-api` | — |
| `cpa-oauth-credential-format` | rewrite | keep-slug | `cpa_import`→`cli_proxy_parser`，4 处路径重取 |
| `rule-51` | keep | rewrite-slug → `five-wire-protocols-anchor` | — |
| `rule-52` | keep | rewrite-slug → `reasoning-content-as-text-block` | — |
| `rule-53` | keep | rewrite-slug → `converter-normalized-intermediate` | — |
| `rule-54` | **drop** | — | 历史 bug 复盘，并入 `wire-protocol-gate-is-failfast` 案例段 |
| `rule-55` | keep | rewrite-slug → `endpoint-cross-protocol-fallback` | — |
| `rule-66` | keep | rewrite-slug → `resolve-price-now-ms-convention` | 新名取自其 frontmatter 既有 `name:` |
| `time-tiers-apply-idiom` | keep | keep-slug | — |
| `trellis-06` | rewrite | rewrite-slug → `mock-platform-contract` | 拦截点改 `handler.rs:412`；`adapter/mock.rs`→`adapter/mock/config.rs` |
| `trellis-07` | rewrite | rewrite-slug → `claude-code-passthrough-platform` | [路径] + `models.rs`→`models/protocol.rs` |
| `trellis-08` | rewrite | rewrite-slug → `platform-auto-disable-codes` | [路径]，验收 grep 更新 |
| `trellis-09` | rewrite | rewrite-slug → `platform-delete-lifecycle` | [路径] |
| `trellis-10` | rewrite | rewrite-slug → `protocol-logo-fallback-chain` | [路径] |

### recall/encoding · frontend · git · i18n（21）

| archive 路径 | 三态 | slug 处置 | 备注 |
|---|---|---|---|
| `encoding/trellis-21` | **drop** | — | 本仓无 `<script type="application/json">` 场景 |
| `frontend/auto-fix-downgrade-37` | keep | rewrite-slug → `tauri-drag-drop-api` | — |
| `frontend/cpa-drag-import-22` | **drop** | — | `dragTargetRef` grep 零命中 |
| `frontend/cpa-drag-import-23` | **drop** | — | `orderLenRef` grep 零命中 |
| `frontend/cpa-drag-import-24` | **drop** | — | `parseInFlightRef` grep 零命中 |
| `frontend/dirty-float-hour-normalization` | rewrite | keep-slug | frontmatter 路径补 `src/` 段 |
| `frontend/form-level-tz-state-sharing` | keep | keep-slug | — |
| `frontend/modal-state-architecture` | rewrite | keep-slug | 跨表单例换 `src/pages/CliProxy/ImportDialog.tsx` |
| `frontend/platform-creation-entry-consolidation` | keep | keep-slug | — |
| `frontend/semantic-token-foreground-pairing` | keep | keep-slug | — |
| `frontend/shadcn-infra-30` | keep | rewrite-slug → `css-var-alias-layer` | — |
| `frontend/shadcn-infra-31` | keep | rewrite-slug → `theme-token-runtime-switch` | — |
| `frontend/tailwind-cascade-layer-unlayered` | keep | keep-slug | 合并 `style/css-reset-layer`（胜出方） |
| `frontend/theme-dark-class-dead-code` | keep | keep-slug | — |
| `frontend/theme/shadcn-primitives-40` | **drop** | — | `sonner.tsx` 已删，冲突消解 |
| `frontend/time-zone-minute-arithmetic` | keep | keep-slug | — |
| `frontend/trellis-18` | rewrite | rewrite-slug → `frontend-conventions` | 补 `src/domains/` 域层 + `api/` 目录；删「组件禁嵌套 >1 层」 |
| `git/rule-44` | keep | rewrite-slug → `parallel-commit-scope-check` | — |
| `i18n/i18n-key-deletion-safety` | keep | keep-slug | — |
| `i18n/rule-04` | keep | rewrite-slug → `i18n-key-eight-locales` | — |
| `i18n/trellis-19` | rewrite | rewrite-slug → `locale-tag-bcp47-consistency` | 行号/路径重取 |

### recall/ops · optimization · proxy · reuse（15，proxy/router 已在 cross-layer 段）

| archive 路径 | 三态 | slug 处置 | 备注 |
|---|---|---|---|
| `ops/buf-residue-observability` | keep | keep-slug | 第三组重复的胜出方 |
| `ops/buffer-residue-no-silent-drop` | **drop** | — | 空壳重复（brief 未点出的第三组） |
| `ops/tauri-logging-guard-lifecycle` | rewrite | keep-slug | 补 guard 实际持有点证据 |
| `ops/trellis-17` | rewrite | rewrite-slug → `remote-defaults-sync-chain` | [路径] |
| `optimization/api-payload-optimization` | keep | keep-slug | — |
| `optimization/manual-budget-empty-shortcircuit` | keep | keep-slug | 行号 189/218 |
| `proxy/rule-50` | rewrite | rewrite-slug → `async-log-queue-backpressure` | [路径] |
| `proxy/sse-chunk-stateless-defect` | keep | keep-slug | — |
| `proxy/trellis-11` | rewrite | rewrite-slug → `connect-tunnel-contract` | [路径] + 分流行号重取；可与 `trellis-13` 合为「非标准 URI 禁走 axum path matcher」 |
| `proxy/trellis-12` | keep | rewrite-slug → `fallback-host-before-path` | [路径] |
| `proxy/trellis-13` | keep | rewrite-slug → `absolute-form-forward-middleware` | [路径] |
| `proxy/trellis-14` | keep | rewrite-slug → `http-client-no-env-proxy` | [路径] |
| `proxy/trellis-15` | keep | rewrite-slug → `diagnostic-header-helper` | [路径] |
| `reuse/auto-fix-downgrade-36` | rewrite | rewrite-slug → `grep-before-write` | 删「扩展 `PROTOCOLS` 数组」条（已改 JSON 派生） |

### recall/shadcn · style · test · testing · ts-rust-boundary（15）

| archive 路径 | 三态 | slug 处置 | 备注 |
|---|---|---|---|
| `shadcn/rule-03` | keep | rewrite-slug → `radix-dialog-requires-title` | — |
| `shadcn/rule-41` | rewrite | rewrite-slug → `radix-select-none-sentinel` | 主用例换 `PlatformPicker.tsx:105-109` |
| `shadcn/rule-42` | keep | rewrite-slug → `radix-select-number-mapping` | — |
| `shadcn/rule-43` | keep | rewrite-slug → `dialog-open-explicit-null` | — |
| `shadcn/rule-45` | rewrite | rewrite-slug → `planning-scope-pregrep` | 去 shadcn 迁移语境，提炼为通用预筛纪律 |
| `shadcn/rule-46` | keep | rewrite-slug → `shadcn-button-svg-size` | — |
| `shadcn/rule-47` | keep | rewrite-slug → `dndkit-migration-keep-logic` | — |
| `style/css-reset-layer` | **drop** | — | 并入 `tailwind-cascade-layer-unlayered` |
| `style/trellis-16` | rewrite | rewrite-slug → `log-format-traceid-contract` | `logging.rs` 留 app 层、gateway 侧 [路径]，两类分别处理 |
| `test/rule-48` | keep | rewrite-slug → `shadcn-test-behavior-assert` | — |
| `test/rule-65` | rewrite | rewrite-slug → `cross-crate-test-path` | 去 arch-deepen-2 语境 |
| `testing/deterministic-pseudorandom-loadgen` | keep | keep-slug | — |
| `testing/module-load-time-constant-test-rule` | keep | keep-slug | — |
| `ts-rust-boundary/mock-config-4layer-consistency` | rewrite | keep-slug | `proxy/config.rs`→`adapter/mock/config.rs` |
| `ts-rust-boundary/optional-config-backward-compat` | rewrite | keep-slug | 同上 |

### rules/（4）— namespace 归位

| archive 路径 | 三态 | slug 处置 | 归位 | 备注 |
|---|---|---|---|---|
| `rules/arch/mock-platform-bypasses-forward-pipeline` | rewrite | keep-slug | → `core/arch/` | `handler.rs:410-429`→`:412`；`anchors:` 三条有效 |
| `rules/perf/hot-path-buffers` | keep | keep-slug | → `core/perf/` | anchors 有效 |
| `rules/perf/stream-buf-no-batching` | keep | keep-slug | → `core/perf/` | 第二组重复的胜出方（采信 recon-backend） |
| `rules/perf/stream-buffer-no-batching-delay` | **drop** | — | — | 空壳重复 |

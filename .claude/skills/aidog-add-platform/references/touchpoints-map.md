# 加平台 / 改平台 — 全文件触点地图

校对于 2026-10-02（registry 化 + crates 拆分后布局）。**以函数名/符号名为准**，行号仅作快速跳转。

图例：✅ 必须 / ⬜ 可选 / 🔺 仅加新 wire 协议时

---

## A. 路径 1 — 普通平台（3 必须）

| # | 触点 | 位置 | 改什么 | 漏改后果 |
|---|---|---|---|---|
| ① | registry platform.json | `src-tauri/defaults/registry/platforms/<code>/platform.json` | endpoints / models / model_list / 品牌字段（name 8 locale / logo_url / color / homepage / keywords / source_urls）+ 可选 key_prefixes / peak / quota_* | 选不到 / 无预填 / check-registry 红 |
| ① | registry 模型文件 | `src-tauri/defaults/registry/platforms/<code>/models/<model>.json` | 每模型一条：model_id / canonical_model / **supported_protocols 必填** / price | check-registry 红 |
| ① | index.json 登记 | `src-tauri/defaults/registry/index.json` | platforms 数组加 `{code, platform_file, models_dir, models[]}` | 远程同步永远拉不到该平台 |
| ① | 盖戳 | `node scripts/bump-registry-last-updated.mjs` | 所有改过的 json 重盖 last_updated（输出数字 > 0 才算盖了） | 远程同步按内容比较跳过 |
| ② | Rust `Protocol` 枚举 | `src-tauri/crates/aidog_db/src/models/protocol.rs`（`pub enum Protocol`） | 对应段落加 `#[serde(rename = "<code>")] Foo,` | 🔴 `from_db_str`（同文件）静默回落 Anthropic：UI 平台类型名错（commandcode 2026-10-01 实锤） |
| ③ | TS `Protocol` 联合类型 | `src/services/api/types/manual.ts`（`export type Protocol =`） | 加 `\| "<code>"`，与 ② rename 逐字一致 | 无编译强制；TS 类型失效 |

> 前端 .ts/.tsx、converter、proxy、router、db 全部 **0 触点**：下拉（`defaults.ts::buildProtocolsFromPresets`）、显示名（`getProtocolLabelMap`）、品牌色（`getProtocolColorMap`）、端点（`getDefaultEndpoints`）、默认模型（`getDefaultModels`）全从 presets 文档派生。

### 可选

| 触点 | 位置 | 何时改 |
|---|---|---|
| 厂商直连端点锁死 | `protocol.rs::endpoints_locked` ↔ `src/domains/platforms/constants.ts::ENDPOINTS_LOCKED_PROTOCOLS`（**同集双写**，有对称测试） | 官方端点固定、禁用户改端点的平台 |
| per-platform wire 微调 | `src-tauri/crates/aidog_adapter/src/<platform>/` + `aidog_adapter/src/lib.rs` mod 声明 | 该平台请求/响应需特调（如 glm）；多数新平台不需要 |
| 余额 / 配额脚本 | platform.json `quota_url_match` + `quota_scripts` | 平台有上游查询 API（见 quota-coding-plan.md） |
| key 前缀识别 | platform.json `key_prefixes` | 平台 API key 有专属前缀（如 sk-ant-） |

---

## B. 加 Protocol 变体后的 Rust match 连锁

实操：加完变体直接 `cargo build`，无 `_` 兜底的 match 编译器指路。

| match / 派生 | 位置 | 兜底? | 加变体后 |
|---|---|---|---|
| fetch models 等 dispatch | `aidog_core/src/platform_cmd/model_fetch.rs`（`matches!` + `_`） | 有 | 多数不补 |
| `convert_request` / `parse_sse` | `aidog_adapter/src/converter/{request,response}.rs` | 有 `_`（按 wire 协议注册表分派） | 不补（平台类型不参与 wire 转换） |
| `inject_coding_plan_fields` | `aidog_core/src/gateway/proxy/headers.rs:440` | 有 `_` | 仅注特殊 body 字段才补（🔴 proxy 与 model-test 两处调用点必须同步） |
| `client_type` 缺省 | `aidog_db/src/registry.rs::derive_client_type` ↔ `src/domains/platforms/defaults.ts::clientTypeForProtocol` | 有 | 不补（跨层对称，禁单侧改） |
| `endpoints_locked` | `protocol.rs:261` ↔ `constants.ts` | — | 仅厂商直连（见 A 可选） |

---

## C. 路径 2 — 加新 wire 协议（A 全部 + 以下）

| # | 触点 | 位置 | 标记 |
|---|---|---|---|
| 1 | 新建 converter（实现 `ProtocolConverter` trait） | `src-tauri/crates/aidog_adapter/src/protocols/<name>/` | 🔺 |
| 2 | converter 注册表 | `aidog_adapter/src/converter/registry.rs::converter_registry` | 🔺 |
| 3 | Protocol 枚举 **wire 段**加变体 | `protocol.rs`「AI 请求协议」段 | 🔺 |
| 4 | wire 鉴权头 | `aidog_core/src/gateway/proxy/`（upstream headers 构造） | 🔺 |
| 5 | 端点协议选项 + 显示名 | `src/domains/platforms/constants.ts` 的 `ENDPOINT_PROTOCOLS` + `PROTOCOL_LABELS`（两处同集） | 🔺 |
| 6 | client_type 缺省派生 | `registry.rs::derive_client_type` ↔ `defaults.ts::clientTypeForProtocol` | 🔺 |
| 7 | model schema 词表 | `src-tauri/defaults/registry/schema/` + `scripts/check-registry.mjs`（supported_protocols 词表两处对称） | 🔺 |

---

## D. coding plan / client_type

| 触点 | 位置 | 说明 |
|---|---|---|
| 独立协议标记 | platform.json 顶层 `is_coding_plan: true`（coding 套餐一律独立 code，**无 JSON 分支**） | Rust `router/ordering.rs` 排序 + 前端徽标派生 |
| `inject_coding_plan_fields` | `aidog_core/src/gateway/proxy/headers.rs:440`（实分支只 Kimi） | 🔴 proxy 与 model-test 调用点必须同步 |
| coding endpoint `client_type` | platform.json endpoints 内显式标注 | 🔴 匹配上游身份白名单（Kimi coding 只接 claude_code） |
| `ClientType` 枚举（全新身份才动） | `aidog_db/src/models/` ↔ TS 对应类型 + 指纹头注入 | 重活，先确认 12 现有变体都不匹配 |

---

## E. 余额 / 配额查询（数据驱动，非代码）

| 触点 | 位置 | 改什么 |
|---|---|---|
| base_url 分派关键词 | platform.json `quota_url_match`（小写子串数组） | 数据驱动，禁代码硬编码（memory `no-hardcoded-platform-words`） |
| 查询脚本 | platform.json `quota_scripts`（变体数组，JS 语法，`http.get` / `ctx.apiKey`） | 返回 `{success, error, balance?, coding_plan?}`；模板见 quota-coding-plan.md |
| 执行 / 物化回落链 | `aidog_db/src/registry.rs`（`resolve_quota_script` / `materialize_quota_script` / `select_quota_variant`） | 0 触点（已泛化） |
| 特例（唯一在代码里的） | `aidog_core/src/platform_cmd/quota.rs`（newapi / devin） | 一般不新增 |
| tier 周期映射 | `aidog_core/src/gateway/usage_color.rs::cycle_ms_for_tier` | 仅全新周期语义才加（配额脚本 tier name 须 ∈ 已知集合） |
| 价格 | registry `models/*.json` 的 `price` 子树（`aidog_db::resolve_price` 消费） | 缺价走 fallback，非 0 |

---

## F. 0 触点（别误改）

| 不该改的 | 原因 |
|---|---|
| 前端预设常量（Platforms.tsx / defaults.ts / constants.ts 的平台列表） | 全部数据驱动派生，不存在硬编码预设 |
| `aidog_db::registry` 合并视图 / presets() | build.rs 自动枚举文件，无需改 Rust |
| db.rs 平台 seed | 不存在 |
| `aidog_adapter/src/<platform>/`（glm / deepseek 等既有目录） | 仅该平台需要 wire 微调才有内容；新平台默认不建 |
| PricingTab / 定价 UI | 定价按模型条目键，与平台无关 |

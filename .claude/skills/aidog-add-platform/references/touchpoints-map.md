# 加平台 / 改平台 — 全文件触点地图

file:line 校对于 2026-06-15。行号随代码漂移，**以函数名/符号名为准**，行号作快速跳转。

图例：✅ 必须 / ⬜ 可选 / 🔺 仅加新 wire 协议时

---

## A. 路径 1 — 纯 OpenAI/Anthropic 兼容平台预设（6 必须）

| # | 触点 | file:line | 改什么 | 漏改后果 |
|---|---|---|---|---|
| ① | `Protocol` 枚举（平台类型段） | `src-tauri/src/gateway/models.rs:5` | 加 `#[serde(rename="foo")] Foo,` | rename = DB/JSON 持久值，不可改 |
| ② | `Protocol` TS 联合类型 | `src/services/api.ts:6-27` | 加 `\| "foo"`（与①逐字一致） | Rust 无容错 → Platform 整体解析失败 |
| ③ | `platform_fetch_models` 的 `\|` 链 | `src-tauri/src/lib.rs:687` | 新变体加进走 `{base}/models` GET 的链 | exhaustive match，cargo build 报错 |
| ④ | `PROTOCOLS` 下拉源 | `src/pages/Platforms.tsx:17` | 加 `{ value, label, keywords, codingPlan? }` | 软失败：下拉里选不到 |
| ⑤ | `getDefaultEndpoints` 预设端点 | `src/pages/Platforms.tsx:150` | 加 `foo: [{ protocol, base_url(含版本前缀), client_type, coding_plan }]` | Partial，软失败：endpoints 空、无预填 base_url |
| ⑥ | `PROTOCOL_LABELS` 显示名 | `src/pages/Platforms.tsx:397` | 加 `foo: "Foo"` | `Record<Protocol,string>` exhaustive，tsc 报错 |

### 可选

| 触点 | file:line | 何时改 |
|---|---|---|
| `PROTOCOL_COLORS` 配色 | `src/pages/Platforms.tsx:472` | 想给平台徽标专属色；key 是 string 非 Protocol，不补走 `var(--accent)` |
| `PROTOCOLS[].keywords` 智能粘贴 | `src/pages/Platforms.tsx:17` + `src/utils/platformPaste.ts`（`matchPlatform`） | 想让智能粘贴识别该平台 |
| 平台图标 | `src/assets/platforms/index.ts` + svg | 想配图标 |
| 默认模型 `getDefaultModels` presets map | `src/pages/Platforms.tsx:371`（见 default-model.md） | 预填模型槽位（强烈建议；消费点 `:1164`/`:1620` 已泛化无需改） |

> 后端 `db.rs` / `router.rs` / `converter.rs` / `adapter/` 路径 1 **均无需改**。

---

## B. 加 Protocol 变体后的 Rust match 连锁

| match | file:line | 兜底? | 加变体后 |
|---|---|---|---|
| `platform_fetch_models` | `src-tauri/src/lib.rs:687` | 无（`\|` 链） | ✅ 必补 |
| `convert_request` | `src-tauri/src/gateway/adapter/converter.rs:10` | 有 `_ => to_openai` | 多数不补（OpenAI 兼容自动兜底） |
| `parse_sse` | `src-tauri/src/gateway/adapter/converter.rs:66` 附近 | 有 `_` | 多数不补 |
| `inject_coding_plan_fields` | `src-tauri/src/gateway/proxy.rs:2553` | 有 `_` | 仅注特殊字段才补 |
| `ClientType::default_for_protocol` | `src-tauri/src/gateway/models.rs:307` | 有 `_` | 多数不补 |

> 实操：加完 `cargo build`，无兜底的 match 编译器指路，按提示补。

---

## C. 路径 2 — 加新 wire 协议（B/路径1 全部 + 以下）

| # | 触点 | file:line | 标记 |
|---|---|---|---|
| 1 | 新建 adapter（参考活的 `adapter/openai.rs`/`anthropic.rs`，**非死代码 glm/kimi**） | `src-tauri/src/gateway/adapter/xxx.rs` | 🔺 |
| 2 | `adapter/mod.rs` 注册 mod | `src-tauri/src/gateway/adapter/mod.rs` | 🔺 |
| 3 | `convert_request` 加 match 分支 | `converter.rs:10` | 🔺 |
| 4 | `parse_sse` 加 match 分支 | `converter.rs:66` 附近 | 🔺 |
| 5 | `parse_incoming_request`（作入站协议时） | `converter.rs` 入站段 | 🔺 |
| 6 | wire 鉴权头 `build_upstream_headers` | `proxy.rs:2482` 附近 | 🔺 |
| 7 | wire 鉴权头 `apply_default_headers` | `proxy.rs:2307` 附近 | 🔺 |
| 8 | `ENDPOINT_PROTOCOLS`（wire 协议进此列表） | `src/pages/Platforms.tsx:91` | 🔺 |
| 9 | `default_for_protocol`（Rust+TS） | `models.rs:307` + `Platforms.tsx:120` | 🔺 |
| 10 | Protocol 枚举里列为 **wire 协议段** | `models.rs:6-16` | 🔺 |

---

## D. coding plan / client_type（仅 coding plan 平台特殊需求）

| 触点 | file:line | 何时改 |
|---|---|---|
| `inject_coding_plan_fields` match 分支 | `src-tauri/src/gateway/proxy.rs:2553`（实分支只 `Protocol::Kimi`） | 仅平台 coding plan 需注特殊 body 字段 |
| proxy 调用点 | `src-tauri/src/gateway/proxy.rs:1074-1077` | 改 inject 逻辑参照 |
| model-test 调用点（parity） | `src-tauri/src/lib.rs:837-838` | 🔴 改 inject 必与 proxy 同步 |
| `override_coding_plan_path`（空壳） | `src-tauri/src/gateway/proxy.rs:2582` | 仅需按平台改 api_path（多数不需要，base_url 已区分） |
| coding endpoint `client_type` | `getDefaultEndpoints`（`Platforms.tsx:150`） | 🔴 须匹配上游身份白名单（Kimi coding 只接 claude_code） |
| `ClientType` 枚举（仅需全新客户端身份时） | `models.rs:270` + `api.ts:41` + `proxy.rs:2250`(apply_client_headers) + `Platforms.tsx:100`(CLIENT_TYPES) | 现有 11 变体都不匹配时才动（重活） |

---

## E. 余额 / 配额查询（仅平台支持上游查询）

| 触点 | file:line | 改什么 |
|---|---|---|
| 余额查询函数 | `src-tauri/src/gateway/quota.rs:138`（`query_deepseek_balance` 模板） | 新增 `query_foo_balance` |
| coding plan 查询函数 | `src-tauri/src/gateway/quota.rs:243`（`query_kimi_coding_plan` 模板） | 新增 `query_foo_coding_plan` |
| 分派（按 base_url 子串） | `src-tauri/src/gateway/quota.rs:373`（`query_quota`，coding 段 ~`:380` 优先 / 余额段 ~`:392`） | 加 `if url.contains("...") { return ... }` |
| tier 周期映射 | `src-tauri/src/gateway/usage_color.rs:30`（`cycle_ms_for_tier`） | 仅全新周期语义才加 name→cycle |
| 价格回退链（不改码） | `src-tauri/src/gateway/db.rs:2840`（`resolve_price`） | 0 触点，填 `model_price.price_data.pricing[platform_type]` |
| 校准落库（不改码） | `src-tauri/src/gateway/estimate.rs:363`（`calibrate_from_quota`） | 已泛化 |
| command（不改码） | `src-tauri/src/lib.rs`（`platform_query_quota`） | 已按 base_url 泛化分派 |
| api.ts（不改码） | `src/services/api.ts`（`quotaApi.query`） | 已封装 |

> 模板细节见 `quota-coding-plan.md`。无上游 quota → `manual_budget.rs` 本地限额兜底，不改 quota.rs。

---

## F. 0 触点（别误改）

| 不该改的 | 原因 |
|---|---|
| `adapter/{glm,kimi,minimax,bailian,codex}.rs` | 死代码 `#[allow(dead_code)]`，从不被 converter dispatch |
| `db.rs` 平台 seed | 不存在，预设住前端 |
| `PricingTab.tsx` | 定价按模型名键，与协议/平台无关（grep 0 命中 Protocol/base_url） |
| `router.rs`（平台选择） | 平台选择走 group_platforms + 调度策略，与 platform_type 解耦；`resolve_model`（`:317`）按槽位名匹配与 Protocol 无关 |

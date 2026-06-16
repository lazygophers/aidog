# Research: 加平台 — 代理层 coding plan 注入 + 前端平台配置 UI 触点

- **Query**: 摸清「代理转发 + coding plan 注入 + 前端平台配置 UI」中与新增/修改平台相关的触点
- **Scope**: internal
- **Date**: 2026-06-15

---

## 0. 核心结论（先读）

- **「纯加一个 OpenAI 兼容平台预设」≠ 加新 Protocol 变体**。绝大多数新平台 = 复用已有 `openai` / `anthropic` wire 协议 + 新的 `Protocol::Xxx` 平台类型枚举（仅作平台主协议，不作 endpoint 协议）。
- **base_url 预设住在前端**（`Platforms.tsx::getDefaultEndpoints`，约 line 150-364），不是后端。后端 `base_url` 永远来自 DB（用户/预设填好后存的字符串），代理层从不硬编码 base_url。
- **加一个 Protocol 枚举变体是「跨 Rust↔TS 双写」契约**：Rust `models.rs::Protocol`（line 5）的 `#[serde(rename="…")]` 字符串 **必须**与 TS `api.ts::Protocol`（line 6-27）联合类型字面量 **逐字一致**，否则反序列化静默回退/失配。
- coding plan 字段注入只有 **1 处真实分支**（Kimi 注 `prompt_cache_key`），api_path 覆盖目前是空壳预留。

---

## 1. 代理层 coding plan 注入

### 1.1 两个函数 + 调用点

| 触点 | file:line | 说明 |
|---|---|---|
| `inject_coding_plan_fields(body, protocol)` 定义 | `src-tauri/src/gateway/proxy.rs:2553` | 按 `platform_protocol`（平台类型枚举）match 注入请求体字段 |
| `override_coding_plan_path(api_path, protocol)` 定义 | `src-tauri/src/gateway/proxy.rs:2582` | 空壳，参数加了 `_` 前缀，**当前不改任何 path**（预留扩展位） |
| 代理调用点 | `src-tauri/src/gateway/proxy.rs:1074-1077` | `if coding_plan { inject…; override…; }` |
| model-test 并行调用点 | `src-tauri/src/lib.rs:835-838` | **parity 要求**：model_test 与 proxy 同步调这两函数。改注入逻辑必须两处一致（见 memory `model-test-proxy-parity`） |

### 1.2 inject 的 match 结构（`proxy.rs:2554-2578`）

```rust
match protocol {                         // protocol = route.platform.platform_type（平台类型，非 wire 协议）
    Protocol::Kimi => { … 注入 prompt_cache_key … }   // proxy.rs:2555
    _ => { /* GLM / MiniMax / 百炼 等暂无额外字段 */ } // proxy.rs:2575
}
```

- **唯一实分支**：`Protocol::Kimi`（line 2555）注 `prompt_cache_key`（5 分钟窗口的 `aidog-{model}-{hex}` 会话 key）。
- **新平台若需特殊 coding plan 字段** → 在 `inject_coding_plan_fields` 的 `match protocol` 内、`Protocol::Kimi` 分支旁、`_` 兜底分支之前，**新增一个 `Protocol::你的新平台 =>` 分支**（`proxy.rs:2554-2578` 区间）。match 的判别值是**平台主协议** `route.platform.platform_type`（line 1059 `platform_protocol`），不是 endpoint 的 wire 协议。
- **新平台若需特殊 coding plan API 路径** → 在 `override_coding_plan_path`（`proxy.rs:2582`）内补 match 分支，并去掉参数的 `_` 前缀。当前各平台靠 `base_url` 区分 coding/normal（如 GLM `…/api/coding/paas/v4` vs `…/api/paas/v4`），所以多数情况**不需要**动这个函数。

### 1.3 注入只在 `coding_plan == true` 时触发

- `coding_plan` 标志来自命中的 endpoint：`proxy.rs:955-957` `matched_ep.map(|ep| (…, ep.coding_plan))`，无匹配 endpoint 时默认 `false`（line 957）。
- 即字段：`PlatformEndpoint.coding_plan: bool`（`models.rs:341`）。前端在 `getDefaultEndpoints` 里给对应 endpoint 打 `coding_plan: cp`（如 `Platforms.tsx:169/177/215`）。

---

## 2. client_type 白名单（协议 ≠ 身份）

### 2.1 概念区分

- **Protocol（协议）**= 报文格式（wire format）：`anthropic` / `openai` / `gemini` 决定 URL path、鉴权 header 名（`x-api-key` vs `Authorization` vs `x-goog-api-key`）、请求体结构。见 `build_upstream_headers` 的 `match protocol`（`proxy.rs:2482-2493`）与 `apply_default_headers`（`proxy.rs:2307-2318`）。
- **ClientType（身份）**= 模拟哪个客户端通过上游校验：注入 User-Agent + `X-Stainless-*` / `originator` / `x-app` 等指纹 header。见 `apply_client_headers`（`proxy.rs:2250`）+ `build_upstream_headers` 的 `match client_type`（`proxy.rs:2495-2538`）。
- 二者正交：同一 `anthropic` 协议可配 `claude_code` 身份（CLI）或 `default`（无指纹）。

### 2.2 「白名单」的真实含义

- 代码里**没有显式 whitelist 表**。所谓白名单 = **上游服务器侧**对 client_type 指纹的校验（aidog 只是模拟以通过它）。
- 关键踩坑（memory `coding-plan-client-type-whitelist`）：**coding plan 上游对 client_type 有身份白名单，协议≠身份**。例：Kimi coding plan 上游**只接 Claude Code 身份、拒 Codex**。所以 `Platforms.tsx:177` 给 Kimi 的 openai-wire coding endpoint 配的是 `client_type: "claude_code"` 而非 `codex_tui`——即便协议是 openai，身份也必须填 claude_code，否则上游拒。

### 2.3 新平台 coding plan 接入注意

1. coding endpoint 的 `client_type` 要匹配**上游实际接受的身份白名单**，而非按 wire 协议想当然（openai 协议未必配 codex 身份）。
2. `ClientType` 是容错反序列化（`models.rs:320-326` `deserialize_client_type_lenient`）：DB 里未知字符串回退 `Default` 而非整个 endpoints 数组解析失败。但回退 = 丢指纹 = 上游可能拒，仍要前端正确填值。
3. 若新平台需要全新的客户端身份（现有 11 个 `ClientType` 变体都不匹配）→ 需在 Rust `ClientType` 枚举（`models.rs:270-299`）+ `apply_client_headers`（`proxy.rs:2256-2275`）+ `build_upstream_headers`（`proxy.rs:2495-2538`）+ UA 表（`claude_code_ua`/`codex_ua` 或新 fn）全加分支，并同步 TS `ClientType`（`api.ts:41-45`）与前端 `CLIENT_TYPES`（`Platforms.tsx:100-117`）。这是重活，纯加平台一般用不上。

---

## 3. 前端 TS 类型同步（跨层契约，错位静默失败）

### 3.1 加 Protocol 变体 = 必须双写

| 层 | file:line | 内容 |
|---|---|---|
| Rust 枚举 | `src-tauri/src/gateway/models.rs:5-` | `Protocol` 变体 + `#[serde(rename="xxx")]` |
| TS 联合类型 | `src/services/api.ts:6-27` | `export type Protocol = … \| "xxx"` |

**契约**：Rust `#[serde(rename = "kimi")]` 的字符串 ↔ TS 字面量 `"kimi"` 必须**逐字一致**。Protocol 在序列化边界（Tauri invoke JSON）传输，不一致会：
- TS 端传一个 Rust 不认识的字符串 → serde 反序列化 `Protocol` 失败（`Protocol` 无容错 deserialize，不像 `ClientType`）→ 整个 Platform 解析失败。
- Rust 端返回一个 TS 没声明的字面量 → TS 编译期不报（运行期 union 收不到），下游 `Record<Protocol,…>` 查表 miss。

### 3.2 Rust 侧加 Protocol 变体的连锁 match（exhaustive，编译强制）

加一个 `Protocol` 变体后，Rust 所有对 `Protocol` 的 exhaustive `match` 都会编译失败，必须补分支。已知热点：
- `src-tauri/src/lib.rs:741` — `platform_fetch_models` 里一长串 `Protocol::A | Protocol::B | … => { /models GET }`。新 OpenAI 兼容平台**多半归到这条 `| ` 链**（走 `{base}/models`）。
- `ClientType::default_for_protocol`（`models.rs:307-313`）— 非 exhaustive（有 `_`），多数情况不用动。
- `build_upstream_headers` / `apply_default_headers` / `apply_*_family_headers`（`proxy.rs:2482` / `2307` / `2345` / `2377`）— 这些 match 的是 **wire 协议**（anthropic/openai/gemini），不是平台类型，新平台类型枚举**不会**触发这里（除非新平台引入新 wire 协议）。
- `inject_coding_plan_fields`（`proxy.rs:2554`）— 有 `_` 兜底，不强制加分支（除非要注字段）。

> 提示：grep `Protocol::` 找所有 exhaustive match。无 `_` 兜底的会编译报错指路；有 `_` 的需人工判断是否要专门分支。

### 3.3 TS 侧加 Protocol 变体的连锁（编译强制 vs 软失败）

- `src/pages/Platforms.tsx:367` `const PROTOCOL_LABELS: Record<Protocol, string>` — **exhaustive Record，TS 编译强制**。加 Protocol 变体不补这里 → `tsc` 报错。**必须补一行**（label）。
- `src/pages/Platforms.tsx:442` `const PROTOCOL_COLORS: Record<string, string>` — key 是 `string` 非 `Protocol`，**非 exhaustive**，不补不报错，运行期 fallback `var(--accent)`（line 777/1127/2004）。可选。
- `src/pages/Platforms.tsx:17` `const PROTOCOLS: ProtocolOption[]` — 平台选择下拉源。**不补 = 新平台不出现在添加下拉里**（软失败，无编译报错）。要让用户能选到，必须加条目。
- `src/pages/Platforms.tsx:150` `getDefaultEndpoints` 的 `base[protocol]` map（`Partial<Record<Protocol,…>>`）— **Partial，非 exhaustive**。不补 = 选了该平台后 endpoints 为空（line 364 `base[protocol] || []`），用户得手填 base_url。要预填 base_url 必须加条目。

---

## 4. 前端 UI：加平台要不要改 Platforms.tsx

### 4.1 「纯加一个 OpenAI 兼容平台预设」(最小改动场景)

判定：新平台只是又一个 OpenAI/Anthropic 兼容中转/聚合站，**复用已有 wire 协议**，只想给它一个独立枚举名 + 预设 base_url。

必须改（缺一即失败）：
1. `models.rs::Protocol`（line 5-）— 加枚举变体 + `#[serde(rename)]`。
2. `api.ts::Protocol`（line 6-27）— 加同名字面量（**与 #1 rename 逐字一致**）。
3. `lib.rs:741` 的 `platform_fetch_models` match — 把新变体加进 `| ` 链（否则 fetch models 编译报错/缺分支）。
4. `Platforms.tsx:367 PROTOCOL_LABELS` — 加 label（tsc exhaustive 强制）。
5. `Platforms.tsx:17 PROTOCOLS` — 加下拉条目（否则选不到）。
6. `Platforms.tsx:150 getDefaultEndpoints` — 加 `base[protocol]` 预设端点（否则无预填 base_url）。

可选：
- `Platforms.tsx:442 PROTOCOL_COLORS` — 配色，不配走 fallback。
- `inject_coding_plan_fields` / `override_coding_plan_path` — 仅当该平台 coding plan 需特殊字段/路径。
- 智能粘贴关键词：`PROTOCOLS` 条目的 `keywords` 数组（解析器靠它匹配，见 `utils/platformPaste.ts:139 matchPlatform` + `SmartPasteModal.tsx`）。

> **注意**：纯加 OpenAI 兼容预设**不需要**改任何 wire 协议 header / converter / 鉴权逻辑——这些按 wire 协议（anthropic/openai/gemini）分支，复用现成。

### 4.2 「加新协议」(需改前后端，重活)

判定：引入一个**新的 wire 协议**（现有 anthropic/openai/openai_responses/openai_completions/gemini 都不匹配上游报文格式）。

除 4.1 全部外，还需：
- `Platforms.tsx:91 ENDPOINT_PROTOCOLS` — 加 endpoint 协议选项（仅 AI 请求协议才进这个列表）。
- `adapter/converter.rs::convert_request`（line ~30-40）+ 新 adapter 模块（参考 `adapter/` 下 15 个 provider adapter）— 实现 wire 转换。
- `build_upstream_headers` / `apply_default_headers` / `apply_*_family_headers`（`proxy.rs:2482`/`2307`/`2345`/`2377`）— 加该协议的鉴权 header 分支。
- `passthrough_api_path`（`converter.rs:54`）+ `parse_sse` / `to_client_sse` — path 与流式解析。
- `ClientType::default_for_protocol`（`models.rs:307` + `Platforms.tsx:120 defaultClientForProtocol`）— 推荐默认身份。

---

## 5. 「加平台」触点清单总表

图例：✅必须 / ⬜可选 / 🔺仅加新 wire 协议时

| # | 触点 | file:line | 纯加 OpenAI 兼容预设 | 加新协议 |
|---|---|---|---|---|
| 1 | `Protocol` 枚举 + serde rename | `models.rs:5` | ✅ | ✅ |
| 2 | `Protocol` TS 联合类型 | `api.ts:6-27` | ✅（rename 逐字一致） | ✅ |
| 3 | `platform_fetch_models` match `\|` 链 | `lib.rs:741` | ✅（编译强制） | ✅ |
| 4 | `PROTOCOL_LABELS` Record | `Platforms.tsx:367` | ✅（tsc exhaustive） | ✅ |
| 5 | `PROTOCOLS` 下拉条目 | `Platforms.tsx:17` | ✅（否则选不到） | ✅ |
| 6 | `getDefaultEndpoints` 预设 base_url | `Platforms.tsx:150` | ✅（否则无预填） | ✅ |
| 7 | `PROTOCOL_COLORS` 配色 | `Platforms.tsx:442` | ⬜ | ⬜ |
| 8 | `PROTOCOLS.keywords` 智能粘贴 | `Platforms.tsx:17` + `platformPaste.ts:139` | ⬜ | ⬜ |
| 9 | `inject_coding_plan_fields` 分支 | `proxy.rs:2554` | ⬜（仅 coding plan 需特殊字段） | ⬜ |
| 10 | `override_coding_plan_path` 分支 | `proxy.rs:2582` | ⬜（base_url 已区分时不需要） | ⬜ |
| 11 | `ENDPOINT_PROTOCOLS` | `Platforms.tsx:91` | 🔺 | ✅ |
| 12 | `convert_request` + 新 adapter | `converter.rs:30` + `adapter/` | 🔺 | ✅ |
| 13 | wire 鉴权 header 分支 | `proxy.rs:2482/2307/2345/2377` | 🔺 | ✅ |
| 14 | `passthrough_api_path` / SSE 解析 | `converter.rs:54` | 🔺 | ✅ |
| 15 | `default_for_protocol`（Rust+TS） | `models.rs:307` + `Platforms.tsx:120` | ⬜ | ✅ |
| 16 | `ClientType` 枚举（仅需全新客户端身份时） | `models.rs:270` + `api.ts:41` + `proxy.rs:2256` + `Platforms.tsx:100` | ⬜ | ⬜ |

**PricingTab 无触点**：`src/pages/PricingTab.tsx` 全文无 `Protocol`/`platform_type`/`base_url`/`coding` 引用（grep 0 命中）。定价按**模型名**键，不按协议/平台键，加平台**不需要**改定价 UI。

---

## 6. 跨 Rust↔TS 边界字段名/类型契约（错位静默失败重点）

| 字段 | Rust | TS | 失配后果 |
|---|---|---|---|
| `Protocol` 字面量 | `models.rs:5` serde rename | `api.ts:6` union | Rust 无容错 deserialize → Platform 整体解析失败 |
| `ClientType` 字面量 | `models.rs:270` serde rename | `api.ts:41` union | Rust **有**容错（`models.rs:320`）→ 回退 `default` → 丢指纹但不崩 |
| `PlatformEndpoint.coding_plan` | `models.rs:341` `bool` | `api.ts:53` `coding_plan?: boolean` | 缺/false → 不触发 inject/override，coding 字段不注入 → 上游可能拒 |
| `PlatformEndpoint.client_type` | `models.rs:338` | `api.ts:52` `client_type?` | 见上，容错回退 |
| `PlatformEndpoint.base_url` | `models.rs:332` `String` | `api.ts:51` `base_url: string` | URL 拼接：`base_url + api_path`，base_url 必须含版本前缀（CLAUDE.md 约束） |

---

## Caveats / Not Found

- `override_coding_plan_path` 当前是空壳（`proxy.rs:2582-2584`），参数全 `_` 前缀，无任何 match 分支——「在哪个 match 分支加」目前**还没有 match**，需要时先建 match 骨架再加分支。
- base_url 预设的「单一事实源」在前端 `getDefaultEndpoints`（`Platforms.tsx:150`），后端不存预设 base_url。这意味着加平台的 base_url 知识点只落前端一处；后端只认 DB 里存好的字符串。
- 本研究未深入 `router.rs` 的平台选择/模型映射逻辑（任务范围限定在 proxy 注入 + 前端 UI + TS 契约），如需路由层触点另查。
- 未逐一枚举 Rust 侧**所有** exhaustive `Protocol` match（除已确认的 `lib.rs:741`）；加变体时以 `cargo build` 报错为准补齐（exhaustive match 编译强制，不会漏）。

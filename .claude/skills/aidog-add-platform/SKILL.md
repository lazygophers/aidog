---
name: aidog-add-platform
description: 在 aidog 里加一个新平台或改一个平台的默认配置（base_url / 端点协议 / coding plan / 默认模型 / 余额查询）。覆盖「加 Protocol 枚举变体（Rust↔TS 双写）、前端 Platforms.tsx 预设、余额/coding plan 配额查询、价格估算接入」全套触点与顺序，并避开调研证实的反直觉陷阱（预设住前端非 db.rs、glm/kimi adapter 是死代码、quota 按 base_url 子串分派）。触发词：加平台、新增平台、添加平台、改平台默认配置、平台 base_url、平台预设、getDefaultEndpoints、获取余额、查余额、coding plan 配额、coding plan 查询、默认模型、Protocol 枚举、新协议、新增 adapter。
when_to_use: 给 aidog 新增一个平台预设（下拉里能选、自动填 base_url）；改某平台的默认 base_url/端点/coding plan；给平台接上游余额或 coding plan 配额查询；加一个全新的 wire 协议（anthropic/openai/gemini 都不匹配）时
paths:
  - src/pages/Platforms.tsx
  - src/services/api.ts
  - src-tauri/src/gateway/**
---

# aidog 加平台 / 改平台

给 aidog 新增一个平台预设，或修改一个平台的默认配置（base_url、端点协议、coding plan、默认模型、余额/配额查询）。本 skill 给出**改哪几处、什么顺序、怎么验证**，并把调研证实的反直觉陷阱前置，避免照源码直觉走错。

> file:line 锚点对应当前代码（2026-06-15 校对）。行号会随代码漂移，定位以**函数名/符号名**为准，行号仅作快速跳转参考。

---

## 0. 四条认知纠偏（动手前必读，最大价值）

1. **平台预设住前端，不在后端。**
   `db.rs` 没有任何平台 seed 函数。`create_platform`（`src-tauri/src/gateway/db.rs:508`）是纯落库——传什么 base_url / endpoints / models 就存什么，**对默认配置零知识**。所有「选了平台 → 自动填 base_url」的预设逻辑都在前端 `src/pages/Platforms.tsx` 的 `getDefaultEndpoints`（`:150`）。**别去 db.rs 找 seed，没有。**

2. **Protocol 枚举 Rust↔TS 必须逐字双写，无容错，失配整体解析失败。**
   Rust `Protocol`（`src-tauri/src/gateway/models.rs:5`）每个变体的 `#[serde(rename="xxx")]` 字符串，必须与 TS 联合类型（`src/services/api.ts:6-27`）的字面量 `"xxx"` **逐字一致**。`Protocol` 没有容错 deserialize（不像 `ClientType` 有 `deserialize_client_type_lenient`，`models.rs:320`）——TS 传一个 Rust 不认识的字符串 → 整个 Platform 反序列化失败而非回退。漏改一侧 = 静默炸。

3. **`adapter/{glm,kimi,minimax,bailian,codex}.rs` 是死代码，禁去改。**
   这 5 个文件全标 `#[allow(dead_code)]`，且 `converter.rs` 的 dispatch **从不调它们**（内部本就 `super::openai::to_openai` 直转）。真实协议转换只在 `convert_request`（`src-tauri/src/gateway/adapter/converter.rs:10`）的 **5 个 wire 分支**。加平台时**不要**去这些 adapter 文件加逻辑。

4. **coding plan / 余额查询按 base_url 子串分派，不按 Protocol 枚举。**
   `query_quota`（`src-tauri/src/gateway/quota.rs:373`）用 `base_url.to_lowercase()` 做 `if url.contains("...")` 顺序匹配，与 `Protocol` 枚举无关。接余额/配额查询时认 base_url 子串，别去枚举上挂。

---

## 1. 先判路径：加平台是哪种？

```
新平台的上游报文格式是什么？
├─ OpenAI Chat Completions 兼容（绝大多数国内/聚合/中转平台）
│     → endpoint.protocol = "openai"，base_url 含 /v1 等前缀
│     → 【路径 1】零 wire 改动，走 converter.rs:34 默认分支 to_openai
├─ Anthropic Messages 兼容（很多平台提供 /anthropic 端点）
│     → endpoint.protocol = "anthropic"，base_url 到 host 根（proxy 拼 /v1/messages）
│     → 【路径 1】零 wire 改动，走 converter.rs:12
├─ Gemini / OpenAI Responses / OpenAI Completions
│     → 对应 wire protocol，已有 adapter，零新增 → 【路径 1】
└─ 全新私有 wire 格式（现实中几乎不出现）
      → 【路径 2】新建 adapter + converter match + parse_sse + 入站解析（重活）
```

90%+ 的「加平台」是**路径 1**——只是又一个 OpenAI/Anthropic 兼容中转/聚合站，复用现成 wire 协议，给它一个独立枚举名 + 预设 base_url。

> 关键区分：**Protocol 变体有两类语义**（`models.rs:6-17` 注释划分）：
> - **wire protocol（可作 endpoint 协议）**：仅 `anthropic / openai / openai_responses / openai_completions / gemini` 这 5 个决定请求体格式与 SSE 解析。
> - **平台类型（仅作平台主协议）**：其余全部（glm/kimi/deepseek/newapi…），只是身份标签 + 决定 OpenAI 兼容平台的 chat path，**不参与 wire 转换**。
> 新平台加的枚举变体几乎总是「平台类型」，wire 复用前 5 个之一。

---

## 2. 路径 1：纯 OpenAI/Anthropic 兼容平台（6 处，缺一即失败）

按顺序改。前 3 处是跨层契约，后 3 处是前端预设。

### ① Rust Protocol 枚举加变体 — `src-tauri/src/gateway/models.rs:5`
在「平台类型」段加一行：
```rust
#[serde(rename = "foo")]   // ★ rename 字符串 = DB/JSON 持久值，确定后不可改
Foo,
```
> 加完后所有对 `Protocol` 的 **exhaustive match** 会编译报错指路（见 §4），按提示补。

### ② TS Protocol 联合类型加字面量 — `src/services/api.ts:6-27`
```ts
| "foo"   // ★ 与 ① 的 rename 逐字一致
```

### ③ `platform_fetch_models` 的 `|` 链 — `src-tauri/src/lib.rs:687`
新 OpenAI 兼容平台多半归到「走 `{base}/models` GET」那条 `Protocol::A | Protocol::B | ... =>` 链。**编译强制**（exhaustive match），不补 `cargo build` 报错。

### ④ `PROTOCOLS` 下拉条目 — `src/pages/Platforms.tsx:17`
```ts
{ value: "foo", label: "Foo", keywords: ["foo", "foo.com"], codingPlan: false },
```
**不补 = 用户在「添加平台」下拉里选不到**（软失败，无编译报错）。`codingPlan: true` 标记编程订阅平台。`keywords` 供智能粘贴解析器匹配（`src/utils/platformPaste.ts` `matchPlatform`）。

### ⑤ `getDefaultEndpoints` 预设端点 — `src/pages/Platforms.tsx:150`
在 `base` map（`Partial<Record<Protocol, PlatformEndpoint[]>>`）里加该平台 → `PlatformEndpoint[]`：
```ts
foo: [
  { protocol: "openai", base_url: "https://api.foo.com/v1", client_type: "default", coding_plan: false },
],
```
**铁律：base_url 含版本前缀**（如 `/v1`、`/api/paas/v4`）。proxy 拼 `api_path`（OpenAI 系 = `/chat/completions`，`converter.rs:44`），**禁额外拼接**。**不补 = 选了平台后 endpoints 为空，用户得手填 base_url**（`Partial`，非 exhaustive，不报错）。

> 多端点示例（glm，`Platforms.tsx:168-171`）：同平台可配 openai + anthropic 双端点，运行期按入站协议匹配（`proxy.rs:945-957`）。coding plan 平台用 `cp` 三元切 base_url（kimi，`:176-178`）。

### ⑥ 显示名 — `src/pages/Platforms.tsx:397` `PROTOCOL_LABELS`
```ts
foo: "Foo",
```
这是 `Record<Protocol, string>`，**tsc exhaustive 强制**，不补 `yarn build` 报错。（研究里把它叫「显示名 map」——它就是 `PROTOCOL_LABELS`；`DEFAULT_NAMES` 在 `:470` 从它派生。）

### 可选项
- **颜色** `PROTOCOL_COLORS`（`Platforms.tsx:472`）：key 是 `string` 非 `Protocol`，非 exhaustive，不补走 fallback `var(--accent)`。
- **默认模型** `getDefaultModels`（`Platforms.tsx:371`）：在 presets map 加一行预填模型槽位，见 §5。强烈建议补（否则选平台后模型槽位空）。
- **余额 / coding plan 查询**：见 `references/quota-coding-plan.md`。

> 路径 1 **不需要**改任何 wire header / converter / 鉴权 / adapter——这些按 wire 协议分支，复用现成。后端 `db.rs / router.rs / converter.rs / adapter/` 均无需动。

完整 file:line 触点表见 `references/touchpoints-map.md`。

---

## 3. 路径 2：加新 wire 协议（重活，几乎用不上）

仅当上游报文格式 anthropic/openai/openai_responses/openai_completions/gemini **全不匹配**时。除 §2 全部外，还需：

1. **新建 adapter** `src-tauri/src/gateway/adapter/xxx.rs`（参考 `adapter/openai.rs` / `anthropic.rs` **活** adapter，**别参考死代码 glm/kimi**）+ `adapter/mod.rs` 注册 mod。
2. **`convert_request` 加 match 分支** — `converter.rs:10`（返回 `(Value, api_path)`）。
3. **`parse_sse` 加 match 分支** — `converter.rs:66` 附近（流式 SSE 解析）。
4. **`parse_incoming_request`**（若作入站协议）— `converter.rs` 入站解析。
5. **wire 鉴权头** — `proxy.rs` `build_upstream_headers`（`:2482` 附近）/ `apply_default_headers`（`:2307` 附近）按 wire 协议加分支（anthropic `x-api-key` / gemini `x-goog-api-key` / 默认 Bearer）。
6. **endpoint 协议选项** `ENDPOINT_PROTOCOLS` — `Platforms.tsx:91`（只有 wire 协议进这个列表）。
7. **默认身份** `ClientType::default_for_protocol`（`models.rs:307`，非 exhaustive 有 `_`）+ 前端 `defaultClientForProtocol`（`Platforms.tsx:120`）。
8. 把新 wire 协议在 Protocol 枚举里**列为 wire 协议段**（`models.rs:6-16` 区）。

---

## 4. 加 Protocol 变体后必补的 Rust match（编译强制）

加一个 `Protocol` 变体后，所有**无 `_` 兜底**的 exhaustive match 编译失败。已知热点：
- `platform_fetch_models`（`lib.rs:687`）— `|` 链，**必补**（见 §2③）。

有 `_` 兜底、**多数情况不用动**的：
- `convert_request` / `parse_sse`（`converter.rs`）— `_ => to_openai`，OpenAI 兼容平台自动兜底。
- `inject_coding_plan_fields`（`proxy.rs:2553`）— `_` 兜底，仅当要注特殊字段才加分支。
- `ClientType::default_for_protocol`（`models.rs:307`）— 有 `_`。

> 实操：加完变体直接 `cargo build`，编译器把所有必补 match 指出来，按提示补 `|` 链即可。不会漏。

---

## 5. 默认模型预设（`getDefaultModels`）

详见 `references/default-model.md`。要点：

预设住前端 `getDefaultModels(protocol, codingPlan?)`（`src/pages/Platforms.tsx:371`），与 `getDefaultEndpoints` 并列。加平台时在其 `presets` map 加一行该平台 → 槽位对象，填**单个 model 名/槽位**（非列表）：
```ts
foo: { default: "foo-model-v1" },                      // OpenAI 兼容平台多归 default 槽
glm: { default: "glm-4.6" },                           // 现有示例
kimi: { default: cp ? "kimi-k2.7-code" : "kimi-k2.6" },  // coding plan 切型号（型号名以源码为准）
```
- 返回 `Partial<Record<ModelSlot, string>>`（Partial，未覆盖平台返回 `{}` 不报错）。
- 槽位 key 必须 ∈ `ModelSlot`（`default/sonnet/opus/haiku/gpt`，`api.ts:47` / `models.rs:235`）。
- 准则：取该平台当前主力型号，**确定才填，不确定留空**（注释 `Platforms.tsx:370`）。
- 两个消费点已按 Protocol 泛化、**无需改**：表单 auto-fill（`:1620`，切协议时 `setModels` 展开预设）、列表卡片回退展示（`:1164`，已配置 → 上游 available → 预设回退）。

路由消费走 `resolve_model`（`router.rs:317`）：请求模型名含 opus/sonnet/haiku/gpt → 用对应槽位，否则 `default`，无 default → 透传（去 `[budget]` 后缀）。

---

## 6. 余额 / coding plan / 价格接入（仅平台支持上游查询时）

完整模板见 `references/quota-coding-plan.md`。要点：

- **余额查询**（按量平台）：照搬 `query_deepseek_balance`（`quota.rs:138`）骨架 → 新增 `query_foo_balance` + 在 `query_quota` 余额段（`quota.rs:392` 附近）加 `if url.contains("api.foo.com") { return query_foo_balance(...).await; }`。返回 `balance: Some(BalanceInfo{...}), coding_plan: None`。
- **coding plan 配额**（订阅平台）：照搬 `query_kimi_coding_plan`（`quota.rs:243`）→ 在 coding plan 段（`quota.rs:380` 附近，**优先于余额段**）加分派。返回 `coding_plan: Some(CodingPlanInfo{tiers, level}), balance: None`。
  - 🔴 **tier `name` 硬约束**：必须 ∈ `cycle_ms_for_tier` 已知集合 `{"five_hour","weekly_limit","seven_day","mcp_monthly"}`（`usage_color.rs:30`）。未知 name → 无周期 → statusline 配色退 Neutral。
- **价格估算**（按量平台）：**无需改代码**。`resolve_price`（`db.rs:2840`）回退链：`pricing[platform_type]` → 顶层 → `default_platform` → fallback。只要 `model_price` 表该模型 `price_data.pricing` 含新 `platform_type` 键（= Protocol rename 字符串）即命中，否则自动回退。靠价格同步/手填，不动代码。
- **无上游 quota API 的平台**：用 `manual_budgets`（platform 列，JSON）本地限额兜底，与请求驱动预估并行（`manual_budget.rs`），无需改 quota.rs。

> 典型新平台只改 `quota.rs`（1 函数 + 1 分派行），可选改 `usage_color.rs`（新 tier 周期）。command（`platform_query_quota`，`lib.rs`）/ api.ts / Platforms.tsx 都已泛化，**无需改**。

---

## 7. client_type 陷阱（协议 ≠ 身份）

- **Protocol** = 报文格式（决定 URL path / 鉴权 header 名 / 请求体结构）。
- **ClientType** = 模拟哪个客户端通过上游校验（注入 UA + `X-Stainless-*` 等指纹头，`proxy.rs:2250` `apply_client_headers`）。
- 二者**正交**：同一 `openai` 协议可配 `codex_tui` 或 `claude_code` 或 `default` 身份。

🔴 **coding plan 上游对 client_type 有身份白名单（协议 ≠ 身份）**：例如 Kimi coding plan 上游**只接 Claude Code 身份、拒 Codex**。所以 `Platforms.tsx` 给 Kimi 的 openai-wire coding endpoint 配的是 `client_type: "claude_code"` 而非 `codex_tui`——即便协议是 openai，身份也必须填上游接受的那个。配 coding endpoint 的 `client_type` 要匹配**上游实际白名单**，别按 wire 协议想当然。

---

## 8. coding plan 字段注入（仅需特殊 body 字段时）

- `inject_coding_plan_fields(body, protocol)`（`proxy.rs:2553`）按**平台主协议** `platform_type` match 注入请求体字段。当前唯一实分支是 `Protocol::Kimi`（注 `prompt_cache_key`），其余走 `_` 兜底（不注）。
- 新平台 coding plan 需特殊字段 → 在 `Protocol::Kimi` 分支旁、`_` 前加 `Protocol::Foo =>` 分支。
- 🔴 **parity 要求**：proxy（`proxy.rs:1074-1077`）与 model-test（`lib.rs:837-838`）**并行**调 `inject_coding_plan_fields` + `override_coding_plan_path`。改注入逻辑**两处必须同步**，否则 model_test 与实际代理行为不一致（见 memory `model-test-proxy-parity`）。
- `override_coding_plan_path`（`proxy.rs:2582`）当前是空壳（参数全 `_` 前缀，无 match）。各平台靠 base_url 区分 coding/normal，多数**不需要**动它；需要时先建 match 骨架。

---

## 9. URL 构造铁律

- `base_url` **含版本前缀**（`/v1`、`/api/paas/v4`、`/api/anthropic` 等）。
- `provider_api_path()`（`converter.rs:44`）OpenAI 系**只返 `/chat/completions`**；anthropic 拼 `/v1/messages`，gemini 拼 `/v1beta/...`。
- 最终 URL = `base_url + api_path`。**禁止额外拼接**（CLAUDE.md 硬约束 + memory `url-construction-rule`）。
- 即 anthropic 端点 base_url 填到 **host 根**（如 `https://open.bigmodel.cn/api/anthropic`，proxy 拼 `/v1/messages`）；openai 端点 base_url 含 `/v1`（proxy 拼 `/chat/completions`）。

---

## 10. 验证门禁

```bash
# 动前端（Platforms.tsx / api.ts）
yarn build            # tsc + vite，exhaustive Record 错位会在这里炸
yarn check:i18n       # 仅当新增了 i18n 文案 key

# 动后端（models.rs / quota.rs / proxy.rs / lib.rs）
cd src-tauri && cargo build          # exhaustive match 漏补在这里炸
cd src-tauri && cargo clippy         # warning 必须清零（memory warnings-are-issues）
cd src-tauri && cargo test           # 若动 quota/estimate/usage_color
```

收尾自检：
- [ ] Protocol 变体 Rust rename ↔ TS 字面量**逐字一致**（§0-2）。
- [ ] `PROTOCOL_LABELS` / `PROTOCOLS` / `getDefaultEndpoints` 三处都补（否则选不到 / 无预填 / tsc 报错）。
- [ ] base_url 含版本前缀，无额外拼接（§9）。
- [ ] coding endpoint `client_type` 匹配上游身份白名单（§7）。
- [ ] coding plan tier `name` ∈ `cycle_ms_for_tier` 集合（§6）。
- [ ] 改 inject 逻辑则 proxy + model-test 两处同步（§8）。
- [ ] 没去改 glm/kimi/minimax/bailian/codex 死代码 adapter（§0-3）。

---

## 反例黑名单（不要做）

1. ❌ 去 `db.rs` 找/加平台 seed —— 预设住前端 `getDefaultEndpoints`。
2. ❌ 只改 Rust Protocol 不改 TS（或反之）—— 无容错，整体解析失败。
3. ❌ 去 `adapter/{glm,kimi,minimax,bailian,codex}.rs` 加转换逻辑 —— 死代码，从不被调。
4. ❌ base_url 不含版本前缀，或在 base_url 后再拼 `/chat/completions` —— 双拼接。
5. ❌ coding endpoint 按 wire 协议想当然填 client_type —— 上游有身份白名单。
6. ❌ coding plan tier 用 `cycle_ms_for_tier` 集合外的 name —— 配色退中性。
7. ❌ 改 inject 只改 proxy 不改 model-test —— parity 破坏。
8. ❌ 加平台去改定价 UI（`PricingTab.tsx`）—— 定价按模型名键，与平台无关，0 触点。

## 相关

- 触点全表：`references/touchpoints-map.md`
- 余额/配额模板：`references/quota-coding-plan.md`
- 默认模型节：`references/default-model.md`
- 请求链路调试：`aidog-request-inspect` skill
- 流程/IA：`aidog-flow-ia` skill

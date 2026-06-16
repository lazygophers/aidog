# Research: aidog 后端平台数据模型 + 预设 + 协议全貌

- **Query**: 摸清「加/改平台」涉及的 Protocol 枚举 / Platform 数据结构 / 平台预设 / 协议转换决策 / 文件触点
- **Scope**: internal（src-tauri/src/gateway + src 前端预设触点）
- **Date**: 2026-06-15

## 关键结论速览（先读这条）

**平台预设（默认 base_url / 协议端点 / coding_plan）不在后端，在前端 `src/pages/Platforms.tsx`。**
后端 `create_platform`（db.rs:508）是纯数据落库，对传入的 base_url / endpoints / models **不做任何默认填充**。"加一个新平台预设" 的主战场是前端 TS 三处枚举 + 一个 `getDefaultEndpoints` 函数；后端只需在 `Protocol` 枚举加一个变体（models.rs），且仅当该平台需要新的 **wire 协议 / 鉴权头 / coding_plan 注入** 时才动后端逻辑。

---

## 1. Protocol 枚举

### 定义位置
`src-tauri/src/gateway/models.rs:5-137`，`pub enum Protocol`，共 ~62 个变体（非任务描述的 168）。
TS 镜像：`src/services/api.ts:6-27`（`export type Protocol = ...` 字面量联合），**必须与 Rust 同步增删**。

### 变体两类语义（models.rs:6-17 注释明确划分）
- **AI 请求协议（wire protocol，可作 endpoint 协议）**：`Anthropic` / `OpenAI` / `OpenAIResponses` / `OpenAICompletions` / `Gemini`（models.rs:7-16）。只有这 5 个决定请求体格式与 SSE 解析。
- **平台类型（platform_type，仅作平台主协议）**：其余全部（Glm/Kimi/DeepSeek/OpenRouter/NewApi/Mock/ClaudeCode…，models.rs:18-136）。它们**不参与 wire 转换**，仅作平台身份标签 + 决定 OpenAI-compatible 平台的 chat path。

### 「每个变体关联的 base_url / api_path / client_type 在哪定义」——关键澄清
**Rust Protocol 枚举本身不携带 base_url / api_path / client_type。** 没有 `impl Protocol { fn base_url() }` 之类方法（已 grep 确认无 `fn base_url` / `fn api_path` / preset fn in models.rs）。映射分散如下：

| 维度 | 定义位置 | 说明 |
|---|---|---|
| base_url（默认值） | 前端 `src/pages/Platforms.tsx:150-360` `getDefaultEndpoints()` | 每个平台类型 → `PlatformEndpoint[]`（含 protocol/base_url/client_type/coding_plan）。后端不存默认 base_url。 |
| api_path | `adapter/converter.rs:10-46` | 由 **wire protocol** 决定：anthropic→`/v1/messages`(:15)、gemini→`/v1beta/models/{model}:streamGenerateContent`(:20)、openai_responses→`/v1/responses`(:26)、openai_completions→`/v1/completions`(:31)、其余(OpenAI 系)→`provider_api_path()`=`/chat/completions`(:44-46)。 |
| client_type | 运行期取自 `PlatformEndpoint.client_type`（models.rs:338），默认值由前端 `defaultClientForProtocol`（Platforms.tsx:120-126）+ `ClientType::default_for_protocol`（models.rs:307-313）给出 |

### 如何加一个新 Protocol 变体
1. `models.rs:5-137` 加 `#[serde(rename="xxx")] Xxx,` 一行（rename 是 DB / JSON 持久值，**确定后不可改**）。
2. `src/services/api.ts:6-27` 同步加 `| "xxx"` 字面量。
3. 若是 OpenAI-compatible 平台（绝大多数情况）：**无需改 converter / proxy 任何 match**，默认 `_` 分支兜底（converter.rs:34-40, parse_sse :71）。
4. 仅当需要专属 wire 格式 / 鉴权头 / coding_plan 注入时才动后端 match（见第 4 节决策树）。

---

## 2. Platform 数据结构

`src-tauri/src/gateway/models.rs:407-477` `pub struct Platform`。TS 镜像 `src/services/api.ts:166+`。全字段：

| 字段 | 类型 | file:line | 语义 |
|---|---|---|---|
| `id` | u64 | models.rs:409 | 主键 |
| `name` | String | :410 | 平台名；空则 db 自动生成 `{type}-{rand8}`（db.rs:512-516） |
| `platform_type` | Protocol | :411 | 平台主协议（身份标签，决 chat path / 透传拦截 / coding 注入路由） |
| `base_url` | String | :412 | 顶层 base_url（**含版本前缀如 /v1**）。endpoints 为空时的回退（proxy.rs:957） |
| `api_key` | String | :413 | 上游密钥 |
| `extra` | String | :414-415 | JSON 额外配置（Mock 配置 / http client 选项等） |
| `models` | PlatformModels | :416-417 | 5 槽位模型映射（见下 §平台默认模型） |
| `available_models` | Vec\<String\> | :418-419 | 从上游 /models 拉取的可用模型列表 |
| `endpoints` | Vec\<PlatformEndpoint\> | :420-422 | **多协议端点**：每协议各自 base_url + client_type + coding_plan。运行期按入站协议匹配（proxy.rs:945-957） |
| `enabled` | bool | :423-425 | 旧布尔启用位，从 status 同步（向后兼容） |
| `status` | PlatformStatus | :426-428 | 三态：enabled/disabled(手动)/auto_disabled(401-403) |
| `auto_disabled_until` | i64 | :429-431 | 退避试探时间戳 |
| `auto_disable_strikes` | i64 | :432-434 | 连续自动禁用次数（指数退避指数） |
| `breaker_failure_threshold` | u32 | :435-437 | 熔断失败阈值，0=继承全局 |
| `breaker_open_secs` | u64 | :438-440 | 熔断 Open 秒数，0=继承全局 |
| `breaker_half_open_max` | u32 | :441-443 | HalfOpen 探测名额，0=继承全局 |
| `created_at`/`updated_at`/`deleted_at` | i64 | :444-447 | 时间戳（deleted_at=0 即未删，软删） |
| `est_balance_remaining` | f64 | :448-450 | 预估剩余余额（系统维护，前端只读） |
| `est_coding_plan` | String | :451-453 | 预估 coding plan JSON（系统维护，前端只读） |
| `last_real_query_at` | i64 | :454-456 | 上次真实 quota 查询时间 |
| `estimate_count` | i64 | :457-459 | 自上次真查的预估次数 |
| `show_in_tray` | bool | :460-462 | 是否 tray 展示 |
| `tray_display` | String | :463-465 | "balance" \| "coding" |
| `sort_order` | i64 | :466-468 | 排序权重 |
| `manual_budgets` | Vec\<ManualBudget\> | :469-471 | 手动预算限额列表 |
| `balance_level` | String | :472-476 | 非 DB 列；list 时按速率算填充，前端配色 |

### 关联结构
- **PlatformModels**（models.rs:235-246）：5 个 `Option<String>` 槽位 `default/sonnet/opus/haiku/gpt`。TS: api.ts:56-62。
- **PlatformEndpoint**（models.rs:330-342）：`protocol: Protocol` + `base_url: String` + `client_type: ClientType`(默认容错反序列化 :337) + `coding_plan: bool`(:340-341)。TS: api.ts:49-54。
- **PlatformStatus**（models.rs:175-204）：三态枚举 + `as_db_str`/`from_db_str`。
- **ClientType**（models.rs:270-299）：Default / Claude Code 家族 5 种 / Codex 家族 4 种 / Cursor / Windsurf。TS: api.ts:41-45。
- **CreatePlatform**（models.rs:479-495）/ **UpdatePlatform**（models.rs:497-517）：入参 DTO。

---

## 3. 平台预设种子（重要：不在后端）

### 后端无种子
- db.rs **没有平台预设 seed 函数**（唯一 seed 是 `seed_builtin_middleware_rules` db.rs:408，与平台无关）。
- `create_platform`（db.rs:508-574）：`models = input.models.unwrap_or_default()`（:517，空），`endpoints = input.endpoints.unwrap_or_default()`（:521，空），base_url 直接落 input（:548）。**后端对平台默认配置零知识**。

### 预设全在前端 `src/pages/Platforms.tsx`
加一个平台预设要改 **4 处**：

| # | 位置 | 内容 |
|---|---|---|
| 1 | Platforms.tsx:17-90 `PROTOCOLS: ProtocolOption[]` | 下拉选项：`{ value, label, codingPlan?, keywords }`。新平台加一行。`codingPlan:true` 标记编程订阅（Platforms.tsx:25/28/40…） |
| 2 | Platforms.tsx:150-360 `getDefaultEndpoints(protocol, codingPlan)` | 核心：平台 → `PlatformEndpoint[]`。每端点 `{protocol(wire), base_url, client_type, coding_plan}`。**base_url 含版本前缀**（如 `/api/paas/v4`、`/v1`），proxy 拼 api_path。例：glm Platforms.tsx:168-171（双端点 openai+anthropic），kimi :176-178（coding_plan 切 base_url） |
| 3 | Platforms.tsx:~430-440 显示名 map | `newapi:"New API"` 等（:435） |
| 4 | Platforms.tsx:~505-511 颜色 map | 平台徽标色（:509-511） |

外加跨层 **Rust+TS Protocol 枚举**（§1）。可选：`src/assets/platforms/index.ts` + svg 加平台图标。

### 「默认填表单」流向
前端选 protocol → `getDefaultEndpoints` 生成 endpoints → 用户填 api_key → `platformApi.create`（CreatePlatform）→ 后端 db.rs:508 原样落库。

---

## 平台默认模型（新增维度）

### 结论：当前**无「平台预设默认模型」机制**
- `PlatformModels`（models.rs:235-246）5 槽位**默认全空**（`Option<String>` 缺省 None）。
- `getDefaultEndpoints`（Platforms.tsx:150-360）只给 **endpoints**，**不给 models**。即新平台预设里没有任何预填模型名。
- 表单初始 models 全空：Platforms.tsx:1534-1535 / 1717 `{ default:"", sonnet:"", opus:"", haiku:"", gpt:"" }`。

### 两种语义现状
1. **展示用默认模型**：无独立"展示默认模型"字段。列表展示的是 `allModelValues(p.models)`（Platforms.tsx:523-533/1128）——即 5 槽位已配置值去重；未配置 → 空，UI 无模型可显示。
2. **表单默认填入模型**：**不预填**。用户须手动填，或点"拉取模型"按钮：`platformApi.fetchModels(protocol, url, apiKey)`（Platforms.tsx:1783）拉上游 /models 列表 → `autoCategorize()`（Platforms.tsx:566-588）按正则自动归槽：opus/sonnet/haiku 正则匹配（:571-573）、gpt（非 mini，:574）、首个未分配项兜底进 `default`（:585-586）。

### 字段名 / 跨层
- Rust：`PlatformModels { default, sonnet, opus, haiku, gpt }`（models.rs:235-246）。
- TS：同名 `PlatformModels`（api.ts:56-62）+ `ModelSlot`（api.ts:47）。
- 路由消费：`resolve_model`（router.rs:317-340）——请求模型名含 "opus/sonnet/haiku/gpt" → 用对应槽位；否则 `default`；无 default → 透传去掉 `[budget]` 后缀。

### 加新平台时默认模型怎么填
- 格式：**单个 model 名/槽位**（非列表），如 `models.opus = "glm-4.6"`。
- 当前预设 **不预填**，靠用户手填或 fetchModels 自动归类。
- 推测: 若要给新平台预置默认模型，现状无承载点——需扩 `getDefaultEndpoints` 或新增 preset models map（当前不存在）。

---

## 4. 协议转换决策树：何时复用 / 何时新增 adapter

### 实际只有 5 个「活」adapter（converter.rs 唯一 dispatch 表）
`convert_request`（converter.rs:10-41）只 match **wire protocol**：
- `Anthropic` → `anthropic::to_anthropic`（converter.rs:12-16）
- `Gemini` → `gemini::to_gemini`（:17-22）
- `OpenAIResponses` → `openai_responses::to_responses`（:23-27）
- `OpenAICompletions` → `openai_completions::to_completions`（:28-32）
- `_`（含 OpenAI + 所有 OpenAI-compatible 平台类型）→ `openai::to_openai`（:34-39）

`parse_sse`（:66-73）同理：anthropic/gemini 专属，其余共用 `openai::parse_openai_sse`。

### **glm/kimi/minimax/bailian/codex adapter 是死代码**
`adapter/{glm,kimi,minimax,bailian,codex}.rs` 全部标 `#[allow(dead_code)]`，且 converter **从不 dispatch 到它们**（已 grep 确认无外部调用：`to_glm/to_kimi/to_minimax/to_bailian/to_codex` 仅在自身文件出现）。这些平台运行时实际走 `_ => to_openai` 默认分支（它们的 endpoint wire protocol 配的是 `openai` 或 `anthropic`）。
- kimi.rs:8-16 / minimax.rs:7-15 / bailian.rs:10-20 / codex.rs:7-14 内部本就 `super::openai::to_openai(req)` 直转。
- glm.rs 有独立 `GlmRequest`（含 web_search 字段）但同样未接入 converter。

### 决策树
```
新平台用什么 wire 协议?
├─ OpenAI Chat Completions 兼容 (绝大多数国内/聚合平台)
│   → endpoint.protocol = "openai"，base_url 含 /v1 等前缀
│   → 零后端改动；走 converter.rs:34 默认分支 to_openai
├─ Anthropic Messages 兼容 (很多平台提供 /anthropic 端点)
│   → endpoint.protocol = "anthropic"，base_url 到 host 根 (proxy 拼 /v1/messages)
│   → 零后端改动；走 converter.rs:12
├─ Gemini / OpenAI Responses / OpenAI Completions
│   → 对应 wire protocol，已有 adapter，零新增
└─ 全新私有 wire 格式 (现实中几乎不出现)
    → 才需: 新建 adapter/xxx.rs + converter.rs 加 match 分支 + parse_sse 加分支
            + (可能) Protocol 枚举把它列为 wire 协议 + proxy parse_incoming_request 入站解析
```

### 何时改后端 proxy.rs（非 adapter）
即使复用 OpenAI 协议，下列平台特性仍需动 proxy.rs match：
- **鉴权头**：默认 Bearer（proxy.rs:2315-2317）；anthropic 用 x-api-key（:2308-2311）、gemini 用 x-goog-api-key（:2312-2314）。按 wire protocol 已覆盖，新平台一般无需改。
- **client_type 模拟头**：Claude Code / Codex / Cursor / Windsurf 家族 UA + Stainless 头（proxy.rs:2325-2458）。
- **coding_plan 注入**：`inject_coding_plan_fields`（proxy.rs:2553-2579）按 platform_type match——目前仅 Kimi 注入 prompt_cache_key（:2555-2574）。新 coding_plan 平台若需特殊 body 字段，在此加 match 分支。
- **透传拦截**：Mock（proxy.rs:881）/ ClaudeCode（:901）特殊路径。

---

## 5. 「加一个新平台」本层文件触点清单 + 改动顺序

### 纯 OpenAI/Anthropic 兼容平台（90% 情况）——只改前端 + 枚举
| 顺序 | 文件:line | 改动 |
|---|---|---|
| 1 | `src-tauri/src/gateway/models.rs:5-137` | Protocol 枚举加 `#[serde(rename="xxx")] Xxx,` |
| 2 | `src/services/api.ts:6-27` | Protocol 联合类型加 `\| "xxx"` |
| 3 | `src/pages/Platforms.tsx:17-90` | PROTOCOLS 加选项行（含 keywords/codingPlan?） |
| 4 | `src/pages/Platforms.tsx:150-360` | getDefaultEndpoints 加平台 → PlatformEndpoint[]（base_url 含版本前缀） |
| 5 | `src/pages/Platforms.tsx:~430-440` | 显示名 map |
| 6 | `src/pages/Platforms.tsx:~505-511` | 颜色 map |
| 7（可选） | `src/assets/platforms/index.ts` + svg | 平台图标 |

后端 db.rs / router.rs / converter.rs / adapter/ **均无需改**。

### 需后端逻辑的平台（新 wire 协议 / coding_plan 特殊注入 / 特殊鉴权）——额外触点
| 场景 | 文件:line |
|---|---|
| coding_plan 注入特殊 body 字段 | `proxy.rs:2553-2579` inject_coding_plan_fields 加 match |
| coding_plan 余额查询 | `quota.rs`（按 base_url 子串 dispatch，:381-389；非 Protocol） |
| 全新 wire 格式 | 新建 `adapter/xxx.rs` + `converter.rs:10-41` convert_request match + `:66-73` parse_sse match + `:78-88` parse_incoming_request（若作入站协议）+ `adapter/mod.rs:1-13` 注册 mod |
| 特殊鉴权头 | `proxy.rs:2302-2320` apply_default_headers / build_upstream_headers :2477-2540 |

### router.rs 对 Protocol 的使用
仅 `resolve_model`（router.rs:317-340，按槽位名匹配，与 Protocol 无关）+ 测试构造。**加平台无需改 router.rs**——平台选择走 group_platforms + 调度策略，与 platform_type 解耦。

---

## Caveats / Not Found

- 任务描述称 "Protocol 枚举 168 变体"，实测约 **62 变体**（models.rs:5-137）。可能把 ClientType(11) + RoutingMode(5) 等其它枚举一并计数，或版本差异。以 models.rs:5-137 实际为准。
- 任务描述称预设种子在 db.rs——**不成立**。db.rs 无平台预设 seed，预设全在前端 Platforms.tsx。这是写 skill 最大的认知纠偏点。
- glm/kimi/minimax/bailian/codex 这 5 个 adapter 文件是 **死代码**（`#[allow(dead_code)]` + 无 dispatch）。写 skill 时不应引导读者去这些文件加逻辑；真实 dispatch 只在 converter.rs 的 5 wire 分支。
- quota.rs coding_plan 查询按 **base_url 子串** dispatch（quota.rs:381-389 判 "kimi"/"zhipu" 等），**不按 Protocol 枚举**——加 coding_plan 平台余额查询时注意这条隐式约定。
- 未深入 lib.rs 的 platform 相关 Tauri command（create_platform/update_platform/fetch_models 等命令封装层）——本研究聚焦 gateway 层 + 前端预设；命令层若需触点可补查 lib.rs。
